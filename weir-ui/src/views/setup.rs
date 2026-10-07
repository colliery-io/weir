//! Setup: onboard connectors and wire a connection.

use crate::components::{config_side, SideConfig};
use crate::discovery::Discovery;
use crate::fetch::get_json;
use crate::helpers::{friendly, is_onboarded, mode_fields, ModeFields, ModeInput};
use crate::models::{AvailableItem, CatalogItem, Connection, ConnectionDetail, PreviewReport, SchemaView};
use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use leptos::prelude::*;
use std::collections::HashSet;

/// The schedule toggle's two choices.
const EVERY: &str = "Every N seconds";
const CRON: &str = "Cron";

/// The connection form's sync/write modes and schedule ([[WEIR-T-0215]]). Health
/// thresholds and the other DTO fields stay API-only.
#[derive(Clone, Copy)]
pub(crate) struct SyncForm {
    pub(crate) sync_mode: RwSignal<String>,
    pub(crate) write_mode: RwSignal<String>,
    /// Comma-separated.
    pub(crate) business_keys: RwSignal<String>,
    pub(crate) cursor_field: RwSignal<String>,
    /// `EVERY` or `CRON`.
    pub(crate) schedule: RwSignal<String>,
    pub(crate) every: RwSignal<String>,
    pub(crate) cron: RwSignal<String>,
    /// Field names of the connection's captured schema, when it has one: the cursor
    /// field is then a select. Empty = a free text input.
    pub(crate) cursor_options: RwSignal<Vec<String>>,
}

impl SyncForm {
    pub(crate) fn new() -> Self {
        Self {
            sync_mode: RwSignal::new("full_refresh".to_string()),
            write_mode: RwSignal::new("append".to_string()),
            business_keys: RwSignal::new(String::new()),
            cursor_field: RwSignal::new(String::new()),
            schedule: RwSignal::new(EVERY.to_string()),
            every: RwSignal::new(String::new()),
            cron: RwSignal::new(String::new()),
            cursor_options: RwSignal::new(Vec::new()),
        }
    }

    /// Fill the form from a stored connection ([[WEIR-T-0216]]).
    pub(crate) fn load(&self, c: &ConnectionDetail) {
        let or = |v: &str, d: &str| if v.is_empty() { d.to_string() } else { v.to_string() };
        self.sync_mode.set(or(&c.sync_mode, "full_refresh"));
        self.write_mode.set(or(&c.write_mode, "append"));
        self.business_keys.set(c.business_keys.join(", "));
        self.cursor_field.set(c.cursor_field.clone().unwrap_or_default());
        self.schedule.set(if c.cron.is_some() { CRON } else { EVERY }.to_string());
        self.every.set(c.every_secs.map(|s| s.to_string()).unwrap_or_default());
        self.cron.set(c.cron.clone().unwrap_or_default());
    }

    /// The fields to send, checked like the server checks them.
    pub(crate) fn fields(&self) -> Result<ModeFields, String> {
        let (sync, write) = (self.sync_mode.get_untracked(), self.write_mode.get_untracked());
        let (keys, cursor) = (self.business_keys.get_untracked(), self.cursor_field.get_untracked());
        let (every, cron) = (self.every.get_untracked(), self.cron.get_untracked());
        let schedule = if self.schedule.get_untracked() == CRON { "cron" } else { "interval" };
        mode_fields(&ModeInput {
            sync_mode: &sync,
            write_mode: &write,
            business_keys: &keys,
            cursor_field: &cursor,
            schedule,
            every: &every,
            cron: &cron,
        })
    }

    /// When the form's name is an existing connection, offer the fields of its captured
    /// schema for the cursor. Discovery gives stream names only, so a new connection has
    /// no schema yet ([[WEIR-T-0218]]).
    pub(crate) fn watch_schema(&self, name: RwSignal<String>, connections: RwSignal<Vec<Connection>>) {
        let options = self.cursor_options;
        let existing = Memo::new(move |_| {
            let n = name.get();
            let n = n.trim();
            connections.with(|cs| cs.iter().any(|c| c.name == n)).then(|| n.to_string())
        });
        Effect::new(move |_| match existing.get() {
            None => options.set(Vec::new()),
            Some(n) => leptos::task::spawn_local(async move {
                let schema = get_json::<SchemaView>(format!("/connections/{n}/schema")).await;
                // The name can change while the request is out: drop a stale answer.
                if existing.get_untracked().as_deref() == Some(n.as_str()) {
                    options.set(schema.fields.into_iter().map(|f| f.name).collect());
                }
            }),
        });
    }
}

/// The modes + schedule part of the connection form.
fn sync_fields(f: SyncForm, exec_mode: RwSignal<String>) -> AnyView {
    let sync_modes = vec![
        ("full_refresh".to_string(), "full refresh · read everything".to_string()),
        ("incremental".to_string(), "incremental · from a cursor field".to_string()),
        ("cdc".to_string(), "cdc · change data capture".to_string()),
    ];
    let write_modes = vec![
        ("append".to_string(), "append · add rows".to_string()),
        ("upsert".to_string(), "upsert · merge on business keys".to_string()),
        ("overwrite".to_string(), "overwrite · replace the table".to_string()),
    ];
    let SyncForm { sync_mode, write_mode, business_keys, cursor_field, schedule, every, cron, cursor_options } = f;
    view! {
        <SimpleGrid cols=2>
            <Select label="Sync mode" option_pairs=sync_modes value=sync_mode/>
            <Select label="Write mode" option_pairs=write_modes value=write_mode/>
        </SimpleGrid>
        {move || (sync_mode.get() == "incremental").then(|| {
            let opts = cursor_options.get();
            if opts.is_empty() {
                view! {
                    <TextInput label="Cursor field *" placeholder="updated_at" value=cursor_field required=true mono=true/>
                }.into_any()
            } else {
                view! {
                    <Select label="Cursor field *" placeholder="— field from the captured schema —" options=opts
                        value=cursor_field required=true/>
                }.into_any()
            }
        })}
        {move || (write_mode.get() == "upsert").then(|| view! {
            <TextInput label="Business keys *" placeholder="id, region" value=business_keys required=true mono=true/>
        })}
        <div>
            <Text size="sm">"Schedule"</Text>
            <SegmentedControl options=vec![EVERY.to_string(), CRON.to_string()] value=schedule/>
        </div>
        {move || if schedule.get() == CRON {
            view! {
                <TextInput label="Cron · 6 or 7 fields, seconds first" placeholder="0 0 * * * *" value=cron mono=true/>
            }.into_any()
        } else {
            let label = if exec_mode.get() == "resident" { "Every (secs) · emit cadence" } else { "Every (secs)" };
            view! { <TextInput label=label placeholder="— none: run on demand" value=every mono=true/> }.into_any()
        }}
    }
    .into_any()
}

/// The stream field: a select of the discovered streams, else a text input; under it
/// what discovery is doing (its error included) and a manual re-run ([[WEIR-T-0218]]).
/// Its own reactive scopes, so a discovery answer does not re-render the form.
fn stream_field(d: Discovery, stream: RwSignal<String>) -> AnyView {
    view! {
        <div>
            {move || {
                let list = d.streams.get();
                if list.is_empty() {
                    view! { <TextInput label="Stream" value=stream mono=true/> }.into_any()
                } else {
                    view! { <Select label="Stream" placeholder="— stream —" options=list value=stream/> }.into_any()
                }
            }}
            <Group gap="sm">
                {move || d.state.get().message().map(|(msg, bad)| {
                    let style = if bad { format!("color: {}", token::BAD) } else { String::new() };
                    view! {
                        <span data-testid="stream-discovery" data-error=bad.to_string() style=style>
                            <Text size="xs" dimmed=!bad>{msg}</Text>
                        </span>
                    }
                })}
                <Button variant="subtle" size="xs" on_click=d.refresh>"Refresh streams"</Button>
            </Group>
        </div>
    }
    .into_any()
}

/// The Setup view's signals + actions.
pub(crate) struct SetupState {
    pub(crate) catalog: RwSignal<Vec<CatalogItem>>,
    pub(crate) available: RwSignal<Vec<AvailableItem>>,
    pub(crate) src_cfg: SideConfig,
    pub(crate) dst_cfg: SideConfig,
    pub(crate) discovery: Discovery,
    pub(crate) add_pkg: RwSignal<String>,
    pub(crate) add_manifest: RwSignal<String>,
    pub(crate) add_path: RwSignal<String>,
    pub(crate) preview: RwSignal<Option<PreviewReport>>,
    pub(crate) name: RwSignal<String>,
    pub(crate) src: RwSignal<String>,
    pub(crate) dst: RwSignal<String>,
    pub(crate) stream: RwSignal<String>,
    pub(crate) sync: SyncForm,
    pub(crate) exec_mode: RwSignal<String>,
    /// The connection being edited ([[WEIR-T-0216]]): its name is read-only.
    pub(crate) editing: RwSignal<Option<String>>,
    pub(crate) cancel_edit: Callback<()>,
    pub(crate) onboard_pick: Callback<()>,
    pub(crate) do_preview: Callback<()>,
    pub(crate) onboard_byo: Callback<()>,
    pub(crate) save_conn: Callback<()>,
}

pub(crate) fn setup_view(s: SetupState) -> AnyView {
    let SetupState {
        catalog, available, src_cfg, dst_cfg, discovery, add_pkg, add_manifest, add_path, preview,
        name, src, dst, stream, sync, exec_mode, editing, cancel_edit,
        onboard_pick, do_preview, onboard_byo, save_conn,
    } = s;
    // Picker options (raw_name, friendly), onboarded dropped.
    let cat = catalog.get();
    let avail = available.get();
    let registered: HashSet<String> = cat.iter().map(|c| c.name.clone()).collect();
    let mut opts: Vec<(String, String)> = Vec::new();
    for want in ["manifest", "dest-manifest", "other"] {
        for p in avail.iter().filter(|p| {
            let k = if want == "other" { p.kind != "manifest" && p.kind != "dest-manifest" } else { p.kind == want };
            k && !is_onboarded(p, &registered)
        }) {
            opts.push((p.name.clone(), friendly(p)));
        }
    }
    let versioned = |c: &CatalogItem| (c.name.clone(), format!("{} · {}", c.name, c.version));
    let sources: Vec<(String, String)> =
        cat.iter().filter(|c| c.roles.iter().any(|r| r == "Source")).map(versioned).collect();
    let dests: Vec<(String, String)> = cat
        .iter()
        .filter(|c| c.roles.iter().any(|r| r == "Destination" || r == "ReverseEtl"))
        .map(versioned)
        .collect();
    let modes = vec![
        ("run_once".to_string(), "run once · scheduled / batch".to_string()),
        ("resident".to_string(), "resident · long-lived (Start/Stop; every = cadence)".to_string()),
    ];

    view! {
        <Panel title="Add a connector" caption="discover · onboard">
            <div class="weir-form">
                <Select label="Pick a connector" placeholder="— select a connector —" option_pairs=opts value=add_pkg/>
                <div><Button on_click=onboard_pick>"Onboard"</Button></div>
            </div>
        </Panel>

        <Panel title="Bring your own" caption="paste a manifest · or a crate path">
            <div class="weir-form">
                <Textarea label="Paste a declarative manifest (YAML)" rows=6 mono=true
                    placeholder="type: DeclarativeSource\nstreams:\n  - ..." value=add_manifest/>
                <TextInput label="Or a crate path (full-code)" placeholder="/path/to/weir-connector-foo"
                    value=add_path mono=true/>
                <Group gap="sm">
                    <Button variant="default" on_click=do_preview>"Preview"</Button>
                    <Button on_click=onboard_byo>"Onboard"</Button>
                </Group>
                {move || preview.get().map(|rep| view! {
                    <Alert title="Preview" color=token::ICE>
                        <Text size="xs" mono=true>
                            {format!("tier {} · confidence {:.2} · streams {}", rep.tier, rep.confidence, rep.streams.len())}
                        </Text>
                        {if rep.unsupported.is_empty() {
                            view! { <Text size="xs" dimmed=true>"Fully supported by the runtime."</Text> }.into_any()
                        } else {
                            view! { <Text size="xs" dimmed=true>{format!("gaps: {}", rep.unsupported.join(", "))}</Text> }.into_any()
                        }}
                    </Alert>
                })}
            </div>
        </Panel>

        <Panel title="New / edit connection" caption="wire a source to a destination">
            <div class="weir-form">
                {move || editing.get().map(|n| view! {
                    <Alert title=format!("Editing {n}") color=token::ICE>
                        <Text size="xs" dimmed=true>
                            "Saving updates this connection. Secrets stay as stored unless you replace or clear them."
                        </Text>
                    </Alert>
                })}
                <TextInput label="Name" placeholder="my-sync" value=name mono=true
                    disabled=Signal::derive(move || editing.get().is_some())/>
                <SimpleGrid cols=2>
                    <Select label="Source" placeholder="— source —" option_pairs=sources value=src/>
                    <Select label="Destination" placeholder="— destination —" option_pairs=dests value=dst/>
                </SimpleGrid>
                <SimpleGrid cols=2>
                    {stream_field(discovery, stream)}
                    <Select label="Execution mode" option_pairs=modes value=exec_mode/>
                </SimpleGrid>
                {sync_fields(sync, exec_mode)}
                // Per-side config ([[WEIR-T-0214]]): sent as `source_config` / `dest_config`.
                {config_side("Source", src, src_cfg)}
                {config_side("Destination", dst, dst_cfg)}
                <Group gap="sm">
                    <Button on_click=save_conn>"Save connection"</Button>
                    {move || editing.get().is_some().then(|| view! {
                        <Button variant="default" on_click=cancel_edit>"Cancel edit"</Button>
                    })}
                </Group>
            </div>
        </Panel>
    }
    .into_any()
}
