//! Setup: onboard connectors and wire a connection.

use crate::components::{config_side, SideConfig};
use crate::helpers::{friendly, is_onboarded};
use crate::models::{AvailableItem, CatalogItem, PreviewReport};
use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use leptos::prelude::*;
use std::collections::HashSet;

/// The Setup view's signals + actions.
pub(crate) struct SetupState {
    pub(crate) catalog: RwSignal<Vec<CatalogItem>>,
    pub(crate) available: RwSignal<Vec<AvailableItem>>,
    pub(crate) src_cfg: SideConfig,
    pub(crate) dst_cfg: SideConfig,
    pub(crate) streams: RwSignal<Vec<String>>,
    pub(crate) add_pkg: RwSignal<String>,
    pub(crate) add_manifest: RwSignal<String>,
    pub(crate) add_path: RwSignal<String>,
    pub(crate) preview: RwSignal<Option<PreviewReport>>,
    pub(crate) name: RwSignal<String>,
    pub(crate) src: RwSignal<String>,
    pub(crate) dst: RwSignal<String>,
    pub(crate) stream: RwSignal<String>,
    pub(crate) every: RwSignal<String>,
    pub(crate) exec_mode: RwSignal<String>,
    pub(crate) onboard_pick: Callback<()>,
    pub(crate) do_preview: Callback<()>,
    pub(crate) onboard_byo: Callback<()>,
    pub(crate) save_conn: Callback<()>,
}

pub(crate) fn setup_view(s: SetupState) -> AnyView {
    let SetupState {
        catalog, available, src_cfg, dst_cfg, streams, add_pkg, add_manifest, add_path, preview,
        name, src, dst, stream, every, exec_mode,
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
    let stream_list = streams.get();
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
                <TextInput label="Name" placeholder="my-sync" value=name mono=true/>
                <SimpleGrid cols=2>
                    <Select label="Source" placeholder="— source —" option_pairs=sources value=src/>
                    <Select label="Destination" placeholder="— destination —" option_pairs=dests value=dst/>
                </SimpleGrid>
                <SimpleGrid cols=2>
                    {if stream_list.is_empty() {
                        view! { <TextInput label="Stream" value=stream mono=true/> }.into_any()
                    } else {
                        view! { <Select label="Stream" placeholder="— stream —" options=stream_list value=stream/> }.into_any()
                    }}
                    {move || {
                        let label = if exec_mode.get() == "resident" { "Every (secs) · emit cadence" } else { "Every (secs)" };
                        view! { <TextInput label=label placeholder="—" value=every mono=true/> }
                    }}
                </SimpleGrid>
                <Select label="Execution mode" option_pairs=modes value=exec_mode/>
                // Per-side config ([[WEIR-T-0214]]): sent as `source_config` / `dest_config`.
                {config_side("Source", src.get_untracked(), src_cfg)}
                {config_side("Destination", dst.get_untracked(), dst_cfg)}
                <div><Button on_click=save_conn>"Save connection"</Button></div>
            </div>
        </Panel>
    }
    .into_any()
}
