//! `weir` — the single-node binary. Thin clap front end over [`weir_cli::App`].

use clap::{Parser, Subcommand};
use weir_app::{App, Connection, connector_ref, plugin_name};

// Bundle the reference connectors into the binary so they resolve by name.

#[derive(Parser)]
#[command(
    name = "weir",
    version,
    about = "weir — no-code data ingestion (single-node)"
)]
struct Cli {
    /// Path to the embedded SQLite store.
    #[arg(long, global = true, default_value = "weir.db")]
    db: String,
    #[command(subcommand)]
    command: Commands,
}

// A CLI command enum is parsed once at startup, so a large variant costs nothing worth
// boxing the clap-derived args for.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Commands {
    /// Create the local store.
    Init,
    /// Manage connections.
    Connection {
        #[command(subcommand)]
        action: ConnAction,
    },
    /// Run a connection once (plan + drain).
    Run {
        #[arg(long)]
        name: String,
    },
    /// Start a resident source (F1) — enqueue-once; a worker runs it indefinitely.
    Start {
        #[arg(long)]
        name: String,
    },
    /// Stop a resident source (F1) — end its supervised restart loop.
    Stop {
        #[arg(long)]
        name: String,
    },
    /// Run the scheduler + worker daemon until ctrl-c.
    Serve {
        /// Poll interval in seconds (fractional; lower bounds the schedule resolution).
        #[arg(long, default_value_t = 0.5)]
        poll: f64,
        /// Max units executing simultaneously (the concurrency cap).
        #[arg(long, default_value_t = 16)]
        concurrency: usize,
    },
    /// Serve the control-plane HTTP API.
    Api {
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Max units executing simultaneously (the concurrency cap).
        #[arg(long, default_value_t = 16)]
        concurrency: usize,
    },
    /// Run a standalone worker — claim + execute from the shared store, no scheduler/HTTP ([[WEIR-I-0021]]).
    Runner {
        /// Poll interval in seconds.
        #[arg(long, default_value_t = 0.5)]
        poll: f64,
        /// Max units executing simultaneously (the concurrency cap).
        #[arg(long, default_value_t = 16)]
        concurrency: usize,
        /// Serve only this tenant's work (pod-per-tenant); unset = all active tenants.
        #[arg(long)]
        tenant: Option<String>,
    },
    /// Run the k8s autoscaler — scale per-tenant runner Deployments on queue depth ([[WEIR-I-0021]]).
    /// Requires a build with `--features kubernetes`.
    Autoscaler {
        /// Poll interval in seconds.
        #[arg(long, default_value_t = 5.0)]
        poll: f64,
        /// Namespace to provision runners in.
        #[arg(long, default_value = "default")]
        namespace: String,
        /// The runner image.
        #[arg(long)]
        image: String,
        /// Min replicas per active tenant.
        #[arg(long, default_value_t = 0)]
        min: i32,
        /// Max replicas per tenant.
        #[arg(long, default_value_t = 8)]
        max: i32,
        /// Queued units one replica is sized to handle.
        #[arg(long, default_value_t = 20)]
        per_replica: i64,
    },
    /// Manage control-plane auth (API keys).
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Print version.
    Version,
}

#[derive(Subcommand)]
enum AuthAction {
    /// Manage API tokens (bearer keys).
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
}

#[derive(Subcommand)]
enum TokenAction {
    /// Mint a new API key — printed once, never recoverable.
    Create {
        #[arg(long)]
        name: String,
        /// Role: read | write | admin.
        #[arg(long, default_value = "admin")]
        role: String,
        /// Tenant id to scope the key to (omit for a global key).
        #[arg(long)]
        tenant: Option<String>,
        /// Platform superuser (cross-tenant god-mode).
        #[arg(long)]
        admin: bool,
    },
    /// List stored keys (metadata only — never the secret).
    List,
    /// Revoke keys matching a prefix or name.
    Revoke {
        /// The key's prefix (last 8 chars) or its name.
        ident: String,
    },
}

// Parsed once at startup; `Add`'s breadth of flags doesn't warrant boxing the clap args.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum ConnAction {
    /// Add (or replace) a connection.
    Add {
        #[arg(long)]
        name: String,
        /// Source connector plugin name (e.g. Rest).
        #[arg(long)]
        source: String,
        /// Destination connector plugin name (e.g. ArrowSink).
        #[arg(long)]
        dest: String,
        #[arg(long)]
        stream: String,
        /// Connector config JSON — the both-sides convenience ([[WEIR-I-0029]]).
        #[arg(long, default_value = "{}")]
        config: String,
        /// Source-only config JSON (overrides `--config` for the source).
        #[arg(long)]
        source_config: Option<String>,
        /// Destination-only config JSON (overrides `--config` for the destination).
        #[arg(long)]
        dest_config: Option<String>,
        /// Optional schedule interval in seconds (fractional, ≥0.1; used by `weir serve`).
        #[arg(long)]
        every: Option<f64>,
        /// Optional cron expression (6/7-field, seconds-first). Takes precedence over `--every`.
        #[arg(long)]
        cron: Option<String>,
        /// How the source reads: full_refresh | incremental | cdc.
        #[arg(long, default_value = "full_refresh")]
        sync_mode: String,
        /// How the destination applies: append | upsert | overwrite.
        #[arg(long, default_value = "append")]
        write_mode: String,
        /// Business keys for upsert (comma-separated).
        #[arg(long)]
        business_keys: Option<String>,
        /// Cursor field for incremental.
        #[arg(long)]
        cursor_field: Option<String>,
        /// Execution mode: run_once (default; batch/CDC) | resident (long-lived source, F1).
        #[arg(long, default_value = "run_once")]
        execution_mode: String,
    },
    /// List connections.
    List,
}

/// Init logging ([[WEIR-A-0022]] / [[WEIR-T-0098]]): structured **JSON** to stderr by default
/// (`WEIR_LOG_FORMAT=pretty` for local dev), with an env filter. Under the `telemetry` feature, if
/// `OTEL_EXPORTER_OTLP_ENDPOINT` is set, also export spans via OTLP.
fn init_tracing() -> anyhow::Result<()> {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let pretty = std::env::var("WEIR_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("pretty"))
        .unwrap_or(false);
    let fmt_layer = if pretty {
        fmt::layer().with_writer(std::io::stderr).boxed()
    } else {
        fmt::layer().json().with_writer(std::io::stderr).boxed()
    };
    let subscriber = tracing_subscriber::registry().with(filter).with(fmt_layer);

    #[cfg(feature = "telemetry")]
    {
        if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
            use anyhow::Context;
            use opentelemetry::trace::TracerProvider;
            let otlp = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .build()
                .context("build OTLP exporter")?;
            let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                .with_batch_exporter(otlp)
                .with_resource(
                    opentelemetry_sdk::Resource::builder()
                        .with_service_name("weir")
                        .build(),
                )
                .build();
            let tracer = provider.tracer("weir");
            subscriber
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .init();
            opentelemetry::global::set_tracer_provider(provider);
        } else {
            subscriber.init();
        }
    }
    #[cfg(not(feature = "telemetry"))]
    subscriber.init();
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing()?;

    let cli = Cli::parse();
    match cli.command {
        Commands::Version => println!("weir {}", env!("CARGO_PKG_VERSION")),
        Commands::Init => {
            let app = App::open(&cli.db)?;
            println!("initialized store at {}", cli.db);
            // Mint the bootstrap admin key on a fresh store (idempotent: no-op if keys exist).
            if let Some(key) = app.bootstrap_admin_key()? {
                println!("\n  admin API key — save this, it is not shown again:\n    {key}\n");
            }
        }
        Commands::Auth { action } => {
            let app = App::open(&cli.db)?;
            match action {
                AuthAction::Token { action } => match action {
                    TokenAction::Create {
                        name,
                        role,
                        tenant,
                        admin,
                    } => {
                        let key = app.create_api_key(&name, &role, tenant.as_deref(), admin)?;
                        let scope = tenant.as_deref().unwrap_or("global");
                        let god = if admin { " admin" } else { "" };
                        println!(
                            "  API key `{name}` ({role}/{scope}{god}) — save this, it is not shown again:\n    {key}"
                        );
                    }
                    TokenAction::List => {
                        for k in app.list_api_keys()? {
                            let last = k.last_used_at.map(|_| "used").unwrap_or("never used");
                            let state = if k.revoked { "revoked" } else { "active" };
                            let scope = k.tenant_id.as_deref().unwrap_or("global");
                            let god = if k.is_admin { " admin" } else { "" };
                            println!(
                                "  {}  {}/{}{}  {state}  ({last})",
                                k.name, k.permissions, scope, god
                            );
                        }
                    }
                    TokenAction::Revoke { ident } => {
                        let n = app.revoke_api_key(&ident)?;
                        println!("revoked {n} key(s) matching `{ident}`");
                    }
                },
            }
        }
        Commands::Connection { action } => {
            let app = App::open(&cli.db)?;
            match action {
                ConnAction::Add {
                    name,
                    source,
                    dest,
                    stream,
                    config,
                    source_config,
                    dest_config,
                    every,
                    cron,
                    sync_mode,
                    write_mode,
                    business_keys,
                    cursor_field,
                    execution_mode,
                } => {
                    let business_keys = business_keys
                        .map(|s| {
                            s.split(',')
                                .map(|k| k.trim().to_string())
                                .filter(|k| !k.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    app.add_connection(
                        weir_app::DEFAULT_TENANT,
                        &Connection {
                            name: name.clone(),
                            source: connector_ref(&source),
                            dest: connector_ref(&dest),
                            stream,
                            // Per-side config ([[WEIR-I-0029]]): the explicit override wins, else `--config`.
                            source_config: source_config.unwrap_or_else(|| config.clone()),
                            dest_config: dest_config.unwrap_or(config),
                            every_secs: every,
                            cron,
                            sync_mode,
                            write_mode,
                            business_keys,
                            cursor_field,
                            execution_mode,
                        },
                    )?;
                    println!("added connection `{name}`");
                }
                ConnAction::List => {
                    for c in app.list_connections(weir_app::DEFAULT_TENANT)? {
                        let sched = match (&c.cron, c.every_secs) {
                            (Some(expr), _) => format!("  cron=\"{expr}\""),
                            (None, Some(s)) => format!("  every={s}s"),
                            _ => String::new(),
                        };
                        println!(
                            "{}  {} -> {}  stream={}{sched}",
                            c.name,
                            plugin_name(&c.source),
                            plugin_name(&c.dest),
                            c.stream
                        );
                    }
                }
            }
        }
        Commands::Run { name } => {
            let app = App::open(&cli.db)?;
            let report = app.run(&name).await?;
            println!("run #{} -> {}", report.work_unit_id, report.state);
        }
        Commands::Start { name } => {
            let app = App::open(&cli.db)?;
            match app.start(weir_app::DEFAULT_TENANT, &name)? {
                Some(id) => println!("started resident `{name}` -> unit #{id}"),
                None => println!("resident `{name}` already running"),
            }
        }
        Commands::Stop { name } => {
            let app = App::open(&cli.db)?;
            let n = app.stop(weir_app::DEFAULT_TENANT, &name)?;
            println!("stopped resident `{name}` ({n} unit(s))");
        }
        Commands::Serve { poll, concurrency } => {
            let app = App::open(&cli.db)?;
            println!("weir serving (db {}); ctrl-c to stop", cli.db);
            app.serve(
                std::time::Duration::from_secs_f64(poll),
                concurrency,
                async {
                    let _ = tokio::signal::ctrl_c().await;
                },
            )
            .await?;
            println!("stopped");
        }
        Commands::Runner {
            poll,
            concurrency,
            tenant,
        } => {
            let app = App::open(&cli.db)?;
            let scope = tenant
                .as_deref()
                .map(|t| format!(" tenant={t}"))
                .unwrap_or_default();
            println!("weir runner (db {}){scope}; ctrl-c to stop", cli.db);
            app.run_workers(
                std::time::Duration::from_secs_f64(poll),
                concurrency,
                tenant,
                async {
                    let _ = tokio::signal::ctrl_c().await;
                },
            )
            .await?;
            println!("stopped");
        }
        Commands::Autoscaler {
            poll,
            namespace,
            image,
            min,
            max,
            per_replica,
        } => {
            #[cfg(feature = "kubernetes")]
            {
                let app = App::open(&cli.db)?;
                let owner = format!("weir-autoscaler-{}", std::process::id());
                println!("weir autoscaler (ns {namespace}); ctrl-c to stop");
                app.run_autoscaler(
                    std::time::Duration::from_secs_f64(poll),
                    owner,
                    namespace,
                    image,
                    cli.db.clone(),
                    weir_app::ScalePolicy {
                        min,
                        max,
                        per_replica,
                    },
                    async {
                        let _ = tokio::signal::ctrl_c().await;
                    },
                )
                .await?;
                println!("stopped");
            }
            #[cfg(not(feature = "kubernetes"))]
            {
                let _ = (poll, namespace, image, min, max, per_replica);
                anyhow::bail!("`weir autoscaler` requires a build with --features kubernetes");
            }
        }
        Commands::Api { port, concurrency } => {
            let app = std::sync::Arc::new(App::open(&cli.db)?);
            println!("weir control-plane API on :{port}");
            weir_api::serve(app, port, concurrency).await?;
        }
    }
    Ok(())
}
