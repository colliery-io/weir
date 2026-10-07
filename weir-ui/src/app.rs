//! The app shell: auth gate, shared state + actions, view routing and the modals.

use crate::components::{Brand, ConnActions, SideConfig};
use crate::fetch::{
    active_tenant, areq_delete, areq_get, areq_post, check, fetch_props, fetch_streams, get_fetch,
    get_json, Fetched,
};
use crate::helpers::{cfg_text, fmt_dur, log_color, side_config};
use crate::models::*;
use crate::views::{health_view, operations_view, platform_view, setup_view, SetupState, SyncForm};
use aurora_leptos::components::*;
use aurora_leptos::theme::{provide_theme, ThemeToggle};
use aurora_leptos::tokens::token;
use aurora_leptos::widgets::Banner;
use aurora_leptos::AuroraStyles;
use leptos::prelude::*;
use std::sync::Arc;

#[component]
pub(crate) fn App() -> impl IntoView {
    // Light / dark / system ([[WEIR-T-0191]]); THEME_INIT_SCRIPT in index.html sets the first paint.
    provide_theme();
    let view = RwSignal::new("Operations".to_string());

    // Auth gate ([[WEIR-T-0087]]): probe `/auth/me` → `authed` (None=checking, Some(false)=needs sign-in).
    let authed = RwSignal::new(Option::<bool>::None);
    let signed_in_as = RwSignal::new(String::new());
    let key_input = RwSignal::new(String::new());
    // Tenant context ([[WEIR-T-0095]]): the signed-in key's tenant + whether it's a platform-admin,
    // and (for admins) the list of tenants for the switcher.
    let my_tenant = RwSignal::new(String::new());
    let is_admin = RwSignal::new(false);
    let tenants = RwSignal::new(Vec::<(String, String)>::new());
    // Tenants admin panel ([[WEIR-T-0096]]): an overlay (admins) to CRUD tenants + their keys.
    let show_tenants = RwSignal::new(false);
    let sel_tenant = RwSignal::new(String::new());
    let tenant_keys = RwSignal::new(Vec::<(String, String, String, bool)>::new()); // (id, name, role, revoked)
    let minted_key = RwSignal::new(Option::<String>::None);
    let new_tenant_id = RwSignal::new(String::new());
    let new_key_name = RwSignal::new(String::new());
    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window().and_then(|w| w.local_storage().ok().flatten())
    }
    let recheck = move || {
        leptos::task::spawn_local(async move {
            match areq_get("/auth/me").send().await {
                Ok(r) if r.ok() => {
                    let me = r.json::<serde_json::Value>().await.unwrap_or_default();
                    signed_in_as.set(me.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string());
                    my_tenant.set(me.get("tenant").and_then(|v| v.as_str()).unwrap_or("default").to_string());
                    let admin = me.get("is_admin").and_then(|v| v.as_bool()).unwrap_or(false);
                    is_admin.set(admin);
                    // A platform-admin can switch tenants → load the list for the switcher.
                    if admin {
                        if let Ok(tr) = areq_get("/tenants").send().await {
                            if tr.ok() {
                                let list = tr.json::<Vec<serde_json::Value>>().await.unwrap_or_default();
                                tenants.set(
                                    list.iter()
                                        .filter_map(|t| {
                                            Some((
                                                t.get("id")?.as_str()?.to_string(),
                                                t.get("name")?.as_str()?.to_string(),
                                            ))
                                        })
                                        .collect(),
                                );
                            }
                        }
                    }
                    authed.set(Some(true));
                }
                _ => authed.set(Some(false)),
            }
        });
    };
    recheck();
    let use_key = move || {
        if let Some(store) = local_storage() {
            let _ = store.set_item("weir_api_key", key_input.get().trim());
        }
        key_input.set(String::new());
        recheck();
    };
    let sign_out = move || {
        if let Some(store) = local_storage() {
            let _ = store.remove_item("weir_api_key");
        }
        leptos::task::spawn_local(async move {
            let _ = areq_get("/auth/logout").send().await;
        });
        authed.set(Some(false));
    };

    let connections = RwSignal::new(Vec::<Connection>::new());
    let runs = RwSignal::new(Vec::<RunRow>::new());
    // Aurora toasts: one queue at the root, one ToastStack in the view (6s, click to dismiss).
    let toaster = provide_toaster();
    let flash = move |ok: bool, msg: String| {
        if ok {
            toaster.success(msg);
        } else {
            toaster.error(msg);
        }
    };
    // Control-plane reachability ([[WEIR-T-0167]]): Some(reason) renders the error banner —
    // an outage must never masquerade as "No connections yet".
    let api_error = RwSignal::new(Option::<String>::None);

    // Tenants admin ([[WEIR-T-0096]]) — CRUD tenants + their keys via /tenants/* (never re-scoped).
    let reload_tenants = move || {
        leptos::task::spawn_local(async move {
            if let Ok(r) = areq_get("/tenants").send().await {
                if r.ok() {
                    let list = r.json::<Vec<serde_json::Value>>().await.unwrap_or_default();
                    tenants.set(list.iter().filter_map(|t| Some((
                        t.get("id")?.as_str()?.to_string(),
                        t.get("name")?.as_str()?.to_string(),
                    ))).collect());
                }
            }
        });
    };
    let load_keys = move |tid: String| {
        sel_tenant.set(tid.clone());
        leptos::task::spawn_local(async move {
            if let Ok(r) = areq_get(&format!("/tenants/{tid}/keys")).send().await {
                if r.ok() {
                    let list = r.json::<Vec<serde_json::Value>>().await.unwrap_or_default();
                    tenant_keys.set(list.iter().filter_map(|k| Some((
                        k.get("id")?.as_str()?.to_string(),
                        k.get("name")?.as_str()?.to_string(),
                        k.get("role")?.as_str()?.to_string(),
                        k.get("revoked").and_then(|v| v.as_bool()).unwrap_or(false),
                    ))).collect());
                }
            }
        });
    };
    let create_tenant = move || {
        let id = new_tenant_id.get().trim().to_string();
        if id.is_empty() { return; }
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "id": id, "name": id });
            let sent = match areq_post("/tenants").json(&body) {
                Ok(r) => check(r.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => flash(true, "tenant created".into()),
                Err(e) => flash(false, format!("create failed: {e}")),
            }
            new_tenant_id.set(String::new());
            reload_tenants();
        });
    };
    let mint_key = move || {
        let tid = sel_tenant.get();
        let name = new_key_name.get().trim().to_string();
        if tid.is_empty() || name.is_empty() { return; }
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "name": name, "role": "write" });
            let sent = match areq_post(&format!("/tenants/{tid}/keys")).json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(r) => {
                    let v = r.json::<serde_json::Value>().await.unwrap_or_default();
                    minted_key.set(v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string()));
                    flash(true, "key minted — copy it now".into());
                    new_key_name.set(String::new());
                    load_keys(tid);
                }
                Err(e) => flash(false, format!("mint failed: {e}")),
            }
        });
    };
    let revoke_key = move |kid: String| {
        let tid = sel_tenant.get();
        leptos::task::spawn_local(async move {
            match check(areq_delete(&format!("/tenants/{tid}/keys/{kid}")).send().await).await {
                Ok(_) => flash(true, "key revoked".into()),
                Err(e) => flash(false, format!("revoke failed: {e}")),
            }
            load_keys(tid);
        });
    };

    // Catalog + available (discover) — refetched on demand after onboard/save.
    let catalog = RwSignal::new(Vec::<CatalogItem>::new());
    let available = RwSignal::new(Vec::<AvailableItem>::new());
    let reload_catalog = move || {
        leptos::task::spawn_local(async move {
            catalog.set(get_json::<Vec<CatalogItem>>("/catalog".into()).await);
            available.set(get_json::<Vec<AvailableItem>>("/catalog/available".into()).await);
        });
    };
    reload_catalog();

    // Connection form state.
    let name = RwSignal::new(String::new());
    let src = RwSignal::new(String::new());
    let dst = RwSignal::new(String::new());
    let stream = RwSignal::new(String::new());
    // Sync/write modes + schedule ([[WEIR-T-0215]]).
    let sync = SyncForm::new();
    sync.watch_schema(name, connections);
    // F1 execution mode ([[WEIR-I-0035]]): run_once (default) | resident.
    let exec_mode = RwSignal::new("run_once".to_string());
    // Edit mode ([[WEIR-T-0216]]): the name of the connection loaded into the form.
    let editing = RwSignal::new(Option::<String>::None);
    // Onboarding state.
    let add_pkg = RwSignal::new(String::new());
    let add_manifest = RwSignal::new(String::new());
    let add_path = RwSignal::new(String::new());
    let preview = RwSignal::new(Option::<PreviewReport>::None);
    // Per-side config ([[WEIR-T-0214]]): each side's contract is refetched when its
    // connector changes; the source's streams are rediscovered too.
    let src_cfg = SideConfig::new();
    let dst_cfg = SideConfig::new();
    let streams = RwSignal::new(Vec::<String>::new());
    Effect::new(move |_| {
        let s = src.get();
        src_cfg.reset();
        leptos::task::spawn_local(async move {
            src_cfg.props.set(fetch_props(&s).await);
            streams.set(fetch_streams(&s, "{}").await);
        });
    });
    Effect::new(move |_| {
        let d = dst.get();
        dst_cfg.reset();
        leptos::task::spawn_local(async move {
            dst_cfg.props.set(fetch_props(&d).await);
        });
    });

    // Per-connection health ([[WEIR-T-0111]]) + the platform rollup ([[WEIR-T-0112]], admin only).
    let health = RwSignal::new(Vec::<ConnHealth>::new());
    let platform = RwSignal::new(PlatformHealth::default());

    // Live poll: /connections + /runs + /overview (health) every 800ms; /platform/health for
    // admins. [[WEIR-T-0167]]: /connections is the canary that classifies auth/server/network
    // state; each fetch only overwrites its signal on success, so an outage shows the error
    // banner over the last-known data instead of fake empty states — and the loop backs off
    // to 3.2s while degraded rather than hammering a dead server.
    leptos::task::spawn_local(async move {
        loop {
            match get_fetch::<Vec<Connection>>("/connections".into()).await {
                Fetched::Ok(v) => {
                    connections.set(v);
                    api_error.set(None);
                }
                Fetched::Unauthorized => {
                    api_error.set(None);
                    authed.set(Some(false));
                }
                Fetched::Failed(s, m) => api_error.set(Some(format!("{m} · HTTP {s}"))),
                Fetched::Network => api_error.set(Some("server unreachable — retrying".into())),
            }
            if let Fetched::Ok(v) = get_fetch::<Vec<RunRow>>("/runs".into()).await {
                runs.set(v);
            }
            if let Fetched::Ok(v) = get_fetch::<Vec<ConnHealth>>("/overview".into()).await {
                health.set(v);
            }
            if is_admin.get_untracked() {
                if let Fetched::Ok(v) = get_fetch::<PlatformHealth>("/platform/health".into()).await
                {
                    platform.set(v);
                }
            }
            let wait = if api_error.get_untracked().is_some() {
                3_200
            } else {
                800
            };
            gloo_timers::future::TimeoutFuture::new(wait).await;
        }
    });

    // Drill from the platform view into a tenant's health ([[WEIR-T-0112]]): re-scope the active
    // tenant (apath then routes /overview → /tenants/{id}/overview) + switch to the Health view.
    let drill_tenant = Callback::new(move |tid: String| {
        if let Some(store) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = store.set_item("weir_active_tenant", &tid);
        }
        view.set("Health".to_string());
    });

    // Run-detail modal.
    let selected = RwSignal::new(String::new());
    let detail_open = RwSignal::new(false);
    let detail_logs = RwSignal::new(Vec::<LogRow>::new());
    let detail_dls = RwSignal::new(Vec::<DeadLetterRow>::new());
    let detail_schema = RwSignal::new(SchemaView::default());
    let open_detail = Callback::new(move |n: String| {
        selected.set(n.clone());
        detail_open.set(true);
        leptos::task::spawn_local(async move {
            detail_logs.set(get_json::<Vec<LogRow>>(format!("/connections/{n}/logs")).await);
            detail_dls.set(get_json::<Vec<DeadLetterRow>>(format!("/connections/{n}/dead-letters")).await);
            detail_schema.set(get_json::<SchemaView>(format!("/connections/{n}/schema")).await);
        });
    });
    // Accept an evolved schema ([[WEIR-T-0121]]): clear the breaking flag, then refetch.
    let accept_schema = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            if let Err(e) = check(
                areq_post(&format!("/connections/{n}/schema/accept"))
                    .send()
                    .await,
            )
            .await
            {
                flash(false, format!("Couldn't accept schema: {e}"));
            }
            detail_schema.set(get_json::<SchemaView>(format!("/connections/{n}/schema")).await);
        });
    });
    let run_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_post(&format!("/connections/{n}/run")).send().await).await {
                Ok(_) => flash(true, format!("Run queued · {n}")),
                Err(e) => flash(false, format!("Couldn't run {n}: {e}")),
            }
        });
    });
    let del_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_delete(&format!("/connections/{n}")).send().await).await {
                Ok(_) => flash(true, format!("Deleted {n}")),
                Err(e) => flash(false, format!("Couldn't delete {n}: {e}")),
            }
        });
    });
    // F1 ([[WEIR-I-0035]]): launch / stop a resident source (enqueue-once + durable stop).
    let start_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_post(&format!("/connections/{n}/start")).send().await).await {
                Ok(_) => {
                    flash(true, format!("Started {n}"));
                    // Refetch immediately so the pill flips without waiting for the 800ms poll.
                    connections.set(get_json::<Vec<Connection>>("/connections".into()).await);
                    runs.set(get_json::<Vec<RunRow>>("/runs".into()).await);
                }
                Err(e) => flash(false, format!("Couldn't start {n}: {e}")),
            }
        });
    });
    let stop_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_post(&format!("/connections/{n}/stop")).send().await).await {
                Ok(_) => {
                    flash(true, format!("Stopped {n}"));
                    // Refetch immediately so the pill flips without waiting for the 800ms poll.
                    connections.set(get_json::<Vec<Connection>>("/connections".into()).await);
                    runs.set(get_json::<Vec<RunRow>>("/runs".into()).await);
                }
                Err(e) => flash(false, format!("Couldn't stop {n}: {e}")),
            }
        });
    });

    // Onboard the selected discover-picker connector.
    let onboard_pick = move || {
        let pkg = add_pkg.get().trim().to_string();
        if pkg.is_empty() {
            flash(false, "Select a connector".into());
            return;
        }
        let kind = available.get_untracked().iter().find(|a| a.name == pkg).map(|a| a.kind.clone()).unwrap_or_default();
        let body = match kind.as_str() {
            "manifest" => serde_json::json!({ "manifest_name": pkg }),
            "dest-manifest" => serde_json::json!({ "dest_manifest_name": pkg }),
            _ => serde_json::json!({ "package": pkg }),
        };
        let label = pkg.clone();
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/catalog/import").json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => {
                    add_pkg.set(String::new());
                    reload_catalog();
                    flash(true, format!("Onboarded {label}"));
                }
                Err(e) => flash(false, format!("Couldn't onboard {label}: {e}")),
            }
        });
    };
    // Preview a pasted manifest.
    let do_preview = move || {
        let m = add_manifest.get().trim().to_string();
        if m.is_empty() {
            flash(false, "Paste a manifest to preview".into());
            return;
        }
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/catalog/preview").json(&serde_json::json!({ "manifest": m }))
            {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(resp) => match resp.json::<PreviewReport>().await {
                    Ok(rep) => preview.set(Some(rep)),
                    Err(e) => flash(false, format!("Couldn't parse preview: {e}")),
                },
                Err(e) => flash(false, format!("Couldn't preview: {e}")),
            }
        });
    };
    // Onboard a pasted manifest or a crate path.
    let onboard_byo = move || {
        let manifest = add_manifest.get().trim().to_string();
        let path = add_path.get().trim().to_string();
        let body = if !manifest.is_empty() {
            serde_json::json!({ "manifest": manifest })
        } else if !path.is_empty() {
            serde_json::json!({ "path": path })
        } else {
            flash(false, "Paste a manifest or enter a crate path".into());
            return;
        };
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/catalog/import").json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => {
                    preview.set(None);
                    add_manifest.set(String::new());
                    add_path.set(String::new());
                    reload_catalog();
                    flash(true, "Onboarded connector".into());
                }
                Err(e) => flash(false, format!("Couldn't onboard: {e}")),
            }
        });
    };
    // Edit ([[WEIR-T-0216]]): load a stored connection into the Setup form. Its secrets
    // arrive as the sentinel, which the form sends back unless the user replaces or
    // clears them; save is the same upsert as create.
    let clear_form = move || {
        editing.set(None);
        name.set(String::new());
        src.set(String::new());
        dst.set(String::new());
        stream.set(String::new());
        exec_mode.set("run_once".to_string());
        sync.load(&ConnectionDetail::default());
    };
    let edit_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match get_fetch::<ConnectionDetail>(format!("/connections/{n}")).await {
                Fetched::Ok(c) => {
                    // Config first: the connector change below resets each side to it.
                    src_cfg.load(cfg_text(&c.source_config));
                    dst_cfg.load(cfg_text(&c.dest_config));
                    editing.set(Some(c.name.clone()));
                    name.set(c.name.clone());
                    src.set(c.source.clone());
                    dst.set(c.dest.clone());
                    stream.set(c.stream.clone());
                    exec_mode.set(if c.execution_mode.is_empty() { "run_once".into() } else { c.execution_mode.clone() });
                    sync.load(&c);
                    view.set("Setup".to_string());
                }
                Fetched::Unauthorized => flash(false, format!("Couldn't load {n}: sign in again")),
                Fetched::Failed(_, e) => flash(false, format!("Couldn't load {n}: {e}")),
                Fetched::Network => flash(false, format!("Couldn't load {n}: server unreachable")),
            }
        });
    });
    // Save the connection.
    let save_conn = move || {
        if name.get().trim().is_empty() {
            flash(false, "Name is required".into());
            return;
        }
        // Each side: form values + the advanced override, required fields checked here.
        let side = |label: &str, c: SideConfig| {
            side_config(label, &c.props.get_untracked(), &c.form.get_untracked(), &c.advanced.get_untracked())
        };
        let (source_config, dest_config) = match (side("Source", src_cfg), side("Destination", dst_cfg)) {
            (Ok(s), Ok(d)) => (s, d),
            (Err(e), _) | (_, Err(e)) => { flash(false, e); return; }
        };
        let modes = match sync.fields() {
            Ok(m) => m,
            Err(e) => { flash(false, e); return; }
        };
        let saved = name.get();
        let body = NewConnection {
            name: name.get(), source: src.get(), dest: dst.get(), stream: stream.get(),
            source_config, dest_config,
            every_secs: modes.every_secs, cron: modes.cron,
            sync_mode: modes.sync_mode, write_mode: modes.write_mode,
            business_keys: modes.business_keys, cursor_field: modes.cursor_field,
            execution_mode: exec_mode.get(),
        };
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/connections").json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => {
                    // An edit is done: the form goes back to a blank new connection, so
                    // its sentinels are not sent for another name.
                    if editing.get_untracked().is_some() {
                        clear_form();
                    }
                    name.set(String::new());
                    flash(true, format!("Saved connection {saved}"));
                }
                // Carries the server's reason — e.g. the [[WEIR-T-0166]] validation messages.
                Err(e) => flash(false, format!("Couldn't save {saved}: {e}")),
            }
        });
    };


    // Confirm-destroy ([[WEIR-T-0191]]): delete + revoke go through Aurora's ConfirmDialog.
    let del_open = RwSignal::new(false);
    let del_name = RwSignal::new(String::new());
    let ask_delete = Callback::new(move |n: String| {
        del_name.set(n);
        del_open.set(true);
    });
    let revoke_open = RwSignal::new(false);
    let revoke_id = RwSignal::new(String::new());

    // Only a definite "not signed in" swaps the app for the sign-in screen; the first
    // probe (None) keeps the shell, so it does not re-render on None → Some(true).
    let needs_signin = Memo::new(move |_| authed.get() == Some(false));

    view! {
        <AuroraStyles/>
        <style>{LAYOUT_CSS}</style>
        <ToastStack duration_ms=6000/>
        {move || if needs_signin.get() {
            // Sign-in gate ([[WEIR-T-0087]]): Aurora's centred sign-in card.
            view! {
                <CenterScreen>
                    <AuthCard title="Sign in to weir" sub="Authenticate to continue."
                        brand=Box::new(|| view! { <Brand/> }.into_any())>
                        <Stack gap="sm">
                            <Button on_click=Callback::new(move |_| {
                                if let Some(w) = web_sys::window() {
                                    let _ = w.location().set_href("/auth/login");
                                }
                            })>"Sign in with OIDC"</Button>
                            <Divider/>
                            <form class="weir-form" on:submit=move |ev: leptos::ev::SubmitEvent| {
                                ev.prevent_default();
                                use_key();
                            }>
                                <PasswordInput label="Or paste an API key" placeholder="weirk_…"
                                    value=key_input autocomplete="off"/>
                                <Button variant="default" button_type="submit">"Use API key"</Button>
                            </form>
                        </Stack>
                    </AuthCard>
                </CenterScreen>
            }.into_any()
        } else {
            view! {
                <AppShell
                    brand=Arc::new(|| view! {
                        <Brand/>
                        <Text size="xs" dimmed=true mono=true>"control plane"</Text>
                    }.into_any())
                    header=Box::new(move || view! {
                        <Group justify="end" gap="sm" wrap=true>
                            <span class="weir-hide-narrow">
                                <Text size="xs" dimmed=true mono=true>{move || format!(
                                    "{} runs · {} rows", runs.get().len(),
                                    runs.get().iter().map(|r| r.rows_written).sum::<i64>())}</Text>
                            </span>
                            // Tenant context ([[WEIR-T-0095]]): a platform-admin gets a switcher; others a chip.
                            // An inline top-bar control with no visible label: `aria_label` names it.
                            {move || if is_admin.get() {
                                let current = RwSignal::new(active_tenant().unwrap_or_default());
                                let pairs = tenants.get().into_iter()
                                    .map(|(id, name)| { let label = format!("{name} · {id}"); (id, label) })
                                    .collect::<Vec<_>>();
                                view! {
                                    <span class="weir-tenant" title="view another tenant">
                                        <Select aria_label="Tenant" placeholder="⊙ self (default)"
                                            option_pairs=pairs value=current
                                            on_change=Callback::new(move |v: String| {
                                                if let Some(store) = local_storage() {
                                                    if v.is_empty() { let _ = store.remove_item("weir_active_tenant"); }
                                                    else { let _ = store.set_item("weir_active_tenant", &v); }
                                                }
                                                if let Some(w) = web_sys::window() { let _ = w.location().reload(); }
                                            })/>
                                    </span>
                                }.into_any()
                            } else {
                                view! { <Pill color=token::MUTED>{move || format!("⊙ {}", my_tenant.get())}</Pill> }.into_any()
                            }}
                            {move || {
                                let mut opts = vec!["Operations".to_string(), "Health".to_string(), "Setup".to_string()];
                                if is_admin.get() { opts.insert(2, "Platform".to_string()); }
                                view! { <SegmentedControl options=opts value=view/> }
                            }}
                            {move || is_admin.get().then(|| view! {
                                <Button variant="subtle" size="xs" title="administer tenants"
                                    on_click=Callback::new(move |_| { reload_tenants(); show_tenants.set(true); })>
                                    "Tenants"
                                </Button>
                            })}
                            {move || view! {
                                <Button variant="subtle" size="xs" title=format!("signed in as {}", signed_in_as.get())
                                    on_click=Callback::new(move |_| sign_out())>"Sign out"</Button>
                            }}
                            <ThemeToggle/>
                        </Group>
                    }.into_any())
                >
                    <div class="weir-page">
                        // Degraded-control-plane banner ([[WEIR-T-0167]]): persistent while the poll fails;
                        // sticky under the top bar so it stays in sight.
                        {move || api_error.get().map(|msg| view! {
                            <div class="weir-alert-slot" data-testid="api-error"
                                title="the dashboards show last-known data until this clears">
                                <Banner color=token::BAD>{format!("control plane error — {msg}")}</Banner>
                            </div>
                        })}
                        {move || match view.get().as_str() {
                            "Setup" => setup_view(SetupState {
                                catalog, available, src_cfg, dst_cfg, streams, add_pkg, add_manifest, add_path, preview,
                                name, src, dst, stream, sync, exec_mode, editing,
                                cancel_edit: Callback::new(move |_| clear_form()),
                                onboard_pick: Callback::new(move |_| onboard_pick()),
                                do_preview: Callback::new(move |_| do_preview()),
                                onboard_byo: Callback::new(move |_| onboard_byo()),
                                save_conn: Callback::new(move |_| save_conn()),
                            }),
                            "Health" => health_view(health, open_detail),
                            "Platform" => platform_view(platform, drill_tenant),
                            _ => operations_view(connections, runs, ConnActions {
                                on_open: open_detail, on_run: run_conn, on_delete: ask_delete,
                                on_start: start_conn, on_stop: stop_conn, on_edit: edit_conn,
                            }),
                        }}
                    </div>

                    <Modal open=detail_open title="Run detail" size="lg">
                        <Text bold=true mono=true>{move || selected.get()}</Text>
                        // Lineage ([[WEIR-T-0101]]): source · stream → dest + rows/duration, from the run data.
                        <SectionLabel label="Lineage" divider=true/>
                        {move || {
                            let name = selected.get();
                            match connections.get().into_iter().find(|c| c.name == name) {
                                Some(c) => {
                                    let run = runs.get().into_iter().find(|r| r.connection == name);
                                    let rows = run.as_ref().map(|r| r.rows_written).unwrap_or(0);
                                    let dur = run.as_ref().and_then(|r| r.duration_ms).map(fmt_dur).unwrap_or_default();
                                    let metrics = if dur.is_empty() {
                                        format!("· {rows} rows")
                                    } else {
                                        format!("· {rows} rows · {dur}")
                                    };
                                    view! {
                                        <Group gap="sm">
                                            <Text size="xs" bold=true mono=true>{c.source}</Text>
                                            <Text size="xs" dimmed=true>{format!("· {} →", c.stream)}</Text>
                                            <Text size="xs" bold=true mono=true>{c.dest}</Text>
                                            <Text size="xs" dimmed=true mono=true>{metrics}</Text>
                                        </Group>
                                    }.into_any()
                                }
                                None => view! { <Text dimmed=true>"—"</Text> }.into_any(),
                            }
                        }}
                        // Typed schema + drift ([[WEIR-T-0121]] / [[WEIR-I-0025]]).
                        <SectionLabel label="Schema" divider=true/>
                        {move || {
                            let sv = detail_schema.get();
                            let name = selected.get();
                            let drift = sv.broken.clone().map(|reason| {
                                let n = name.clone();
                                view! {
                                    <Alert title="Schema drift" color=token::BAD>
                                        <Stack gap="xs">
                                            <Text size="sm">{reason}</Text>
                                            <div>
                                                <Button variant="default" size="xs"
                                                    on_click=Callback::new(move |_| accept_schema.run(n.clone()))>
                                                    "Accept new schema"
                                                </Button>
                                            </div>
                                        </Stack>
                                    </Alert>
                                }
                            });
                            let body = if sv.fields.is_empty() {
                                view! { <Text size="sm" dimmed=true>"No schema captured yet."</Text> }.into_any()
                            } else {
                                view! {
                                    <DetailList mono=true>
                                        {sv.fields.into_iter().map(|f| {
                                            let opt = if f.nullable { "nullable" } else { "required" };
                                            view! { <KeyValue label=f.name>{format!("{} · {opt}", f.ty)}</KeyValue> }
                                        }).collect_view()}
                                    </DetailList>
                                }.into_any()
                            };
                            view! { <Stack gap="sm">{drift}{body}</Stack> }.into_any()
                        }}
                        <SectionLabel label="Dead-letters" count=Signal::derive(move || Some(detail_dls.get().len())) divider=true/>
                        {move || {
                            let dls = detail_dls.get();
                            if dls.is_empty() {
                                view! { <Text size="sm" dimmed=true>"None."</Text> }.into_any()
                            } else {
                                view! {
                                    <Table mono=true fixed=true widths=vec!["38%".into(), "62%".into()] label="Dead-letters">
                                        <thead><tr><th>"reason"</th><th>"record"</th></tr></thead>
                                        <tbody>
                                            {dls.into_iter().map(|d| view! {
                                                <tr><td title=d.reason.clone()>{d.reason.clone()}</td><td title=d.record.clone()>{d.record.clone()}</td></tr>
                                            }).collect_view()}
                                        </tbody>
                                    </Table>
                                }.into_any()
                            }
                        }}
                        <SectionLabel label="Logs" divider=true/>
                        <LogView label="Logs" empty="No logs." max_height="260px"
                            lines=Signal::derive(move || detail_logs.get().into_iter()
                                .map(|l| LogLine::new(l.message).level_color(l.level.clone(), log_color(&l.level)))
                                .collect::<Vec<_>>())/>
                    </Modal>

                    // Tenants admin ([[WEIR-T-0096]]) — platform-admin CRUD tenants + their keys.
                    <Modal open=show_tenants title="Tenants" size="lg" close_on_scrim=false on_close=Callback::new(move |_| minted_key.set(None))>
                        <Stack>
                            <Text size="sm" dimmed=true>"Administer tenants + their keys."</Text>
                            <TextInput label="New tenant id" placeholder="acme" value=new_tenant_id mono=true/>
                            <div><Button on_click=Callback::new(move |_| create_tenant())>"Create tenant"</Button></div>
                            <Table label="Tenants">
                                <thead><tr><th>"tenant"</th><th>"name"</th><th></th></tr></thead>
                                <tbody>
                                    {move || tenants.get().into_iter().map(|(id, name)| {
                                        let (id2, id3) = (id.clone(), id.clone());
                                        view! {
                                            <TableRow label=format!("keys of {id}") selected=Signal::derive(move || sel_tenant.get() == id3)
                                                on_click=Callback::new(move |_| { minted_key.set(None); load_keys(id2.clone()); })>
                                                <td><Code>{id.clone()}</Code></td>
                                                <td>{name}</td>
                                                <td class="cl-num"><Text size="xs" dimmed=true>"Keys →"</Text></td>
                                            </TableRow>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </Table>
                            {move || (!sel_tenant.get().is_empty()).then(|| view! {
                                <SectionLabel label=format!("Keys · {}", sel_tenant.get()) divider=true/>
                                {move || minted_key.get().map(|k| view! {
                                    <SecretReveal secret=k label="New API key"
                                        on_done=Callback::new(move |_| minted_key.set(None))/>
                                })}
                                <TextInput label="New key name" placeholder="ci" value=new_key_name/>
                                <div><Button on_click=Callback::new(move |_| mint_key())>"Mint key"</Button></div>
                                <Table label="Keys">
                                    <thead><tr><th>"name"</th><th>"role"</th><th>"state"</th><th></th></tr></thead>
                                    <tbody>
                                        {move || tenant_keys.get().is_empty().then(|| view! {
                                            <TableEmpty message="No keys yet." colspan=4/>
                                        })}
                                        {move || tenant_keys.get().into_iter().map(|(kid, name, role, revoked)| {
                                            view! { <tr>
                                                <td>{name}</td><td>{role}</td>
                                                <td>{if revoked {
                                                    view! { <Pill color=token::MUTED>"revoked"</Pill> }.into_any()
                                                } else {
                                                    view! { <Pill color=token::OK>"active"</Pill> }.into_any()
                                                }}</td>
                                                <td class="cl-num">{(!revoked).then(|| view! {
                                                    <Button variant="subtle" size="xs" bad=true
                                                        on_click=Callback::new(move |_| { revoke_id.set(kid.clone()); revoke_open.set(true); })>
                                                        "Revoke"
                                                    </Button>
                                                })}</td>
                                            </tr> }
                                        }).collect_view()}
                                    </tbody>
                                </Table>
                            })}
                        </Stack>
                    </Modal>

                    <ConfirmDialog open=del_open title=Signal::derive(move || format!("Delete {}?", del_name.get())) confirm_label="Delete"
                        message="This removes the connection and stops its schedule. Its run history stays."
                        on_confirm=Callback::new(move |_| { del_conn.run(del_name.get_untracked()); del_open.set(false); })>
                        <Text mono=true bold=true>{move || del_name.get()}</Text>
                    </ConfirmDialog>
                    <ConfirmDialog open=revoke_open title=Signal::derive(move || format!("Revoke key {}?", revoke_id.get())) confirm_label="Revoke"
                        message="A revoked key stops working at once. This cannot be undone."
                        on_confirm=Callback::new(move |_| { revoke_key(revoke_id.get_untracked()); revoke_open.set(false); })/>
                </AppShell>
            }.into_any()
        }}
    }
}

/// What Aurora does not do for weir: the brand mark, the page width, the sticky error
/// slot, the connection-flow arrow and the form rhythm. Every colour is an Aurora token.
pub(crate) const LAYOUT_CSS: &str = r#"
.weir-brand { display: inline-flex; align-items: baseline; gap: 6px; }
.weir-glyph, .weir-wordmark { font-family: var(--font-mono); font-weight: 700;
  background: var(--aurora-3); -webkit-background-clip: text; background-clip: text; color: transparent; }
.weir-glyph { font-size: 19px; } .weir-wordmark { font-size: 20px; letter-spacing: .05em; }
.weir-page { max-width: 1180px; margin: 0 auto; display: grid; gap: var(--space-lg); }
/* grid items keep the page width: a wide table scrolls in its own box. */
.weir-page > * { min-width: 0; }
.weir-alert-slot { position: sticky; top: var(--cl-header-h); z-index: 30; }
.weir-arr { color: var(--ice); font-family: var(--font-mono); font-size: var(--fs-xs); }
.weir-form { display: grid; gap: var(--space-md); }
.weir-tenant .cl-select { width: auto; max-width: 220px; height: var(--h-xs); font-size: var(--fs-xs); }
@media (max-width: 768px) {
  .cl-appshell__header { flex-wrap: wrap; }
  .cl-appshell__header-content { flex-basis: 100%; }
  .weir-hide-narrow { display: none; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_html_carries_the_theme_init_script() {
        let html = include_str!("../index.html");
        assert!(html.contains(aurora_leptos::THEME_INIT_SCRIPT), "index.html must inline THEME_INIT_SCRIPT");
        assert!(html.contains(r#"name="color-scheme" content="light dark""#));
    }

    #[test]
    fn layout_css_uses_only_defined_tokens_and_no_raw_colour() {
        for bad in ["--bg-2", "--fg-1", "--radius-sm4", "rgba(", "rgb(", "#"] {
            assert!(!LAYOUT_CSS.contains(bad), "LAYOUT_CSS contains {bad}");
        }
    }
}
