//! Shared widgets: the brand mark, the connection card, a config-contract field.

use crate::helpers::{cfg_get, cfg_set, run_metrics, state_color};
use crate::models::{Connection, Prop, RunRow};
use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use leptos::prelude::*;

/// The weir mark: the gradient glyph + wordmark (brand stays in the product).
#[component]
pub(crate) fn Brand() -> impl IntoView {
    view! {
        <span class="weir-brand">
            <span class="weir-glyph">"≋"</span>
            <span class="weir-wordmark">"weir"</span>
        </span>
    }
}

/// One field of a connector's config contract, bound both ways to the config JSON.
pub(crate) fn config_field(p: Prop, config: RwSignal<String>) -> impl IntoView {
    let field = RwSignal::new(cfg_get(&config.get_untracked(), &p.key));
    let (k_sync, k_set, kind_sync, kind_set) = (p.key.clone(), p.key.clone(), p.kind.clone(), p.kind.clone());
    // The JSON textarea can change the config too: follow it, unless the config already
    // holds what this field's text means (so typing "1." is not rewritten to "1").
    Effect::new(move |_| {
        let now = cfg_get(&config.get(), &k_sync);
        let mine = field.get_untracked();
        if now != cfg_get(&cfg_set("{}", &k_sync, &mine, &kind_sync), &k_sync) {
            field.set(now);
        }
    });
    let on_input = Callback::new(move |v: String| {
        config.set(cfg_set(&config.get_untracked(), &k_set, &v, &kind_set));
    });
    let input_type = if p.secret { "password" } else { "text" };
    view! { <TextInput label=p.key placeholder=p.kind value=field input_type=input_type on_input=on_input mono=true/> }
}

/// What a connection card can do.
#[derive(Clone, Copy)]
pub(crate) struct ConnActions {
    pub(crate) on_open: Callback<String>,
    pub(crate) on_run: Callback<String>,
    pub(crate) on_delete: Callback<String>,
    pub(crate) on_start: Callback<String>,
    pub(crate) on_stop: Callback<String>,
}

#[component]
pub(crate) fn ConnectionCard(conn: Connection, last: Option<RunRow>, actions: ConnActions) -> impl IntoView {
    let state = last.as_ref().map(|r| r.state.clone()).unwrap_or_else(|| "idle".to_string());
    // The run id is a long snowflake: it goes in the tooltip, the metrics stay in sight.
    let (when, when_title) = match &last {
        Some(r) => (run_metrics(r), format!("run #{}", r.id)),
        None => ("no runs yet".to_string(), String::new()),
    };
    // F1 ([[WEIR-I-0035]]): a resident source shows a live/stopped badge + Start/Stop instead of Run.
    // "live" = it holds an active (leased/pending) run; otherwise it's stopped.
    let resident = conn.execution_mode == "resident";
    let live = matches!(state.as_str(), "leased" | "pending");
    let (n_open, n_run, n_del, n_start, n_stop) = (
        conn.name.clone(), conn.name.clone(), conn.name.clone(), conn.name.clone(), conn.name.clone(),
    );
    let (pill_color, pill_label) = if resident {
        if live { (token::OK, "resident • live".to_string()) } else { (token::MUTED, "resident • stopped".to_string()) }
    } else {
        (state_color(&state), state.clone())
    };
    view! {
        <Card label=conn.name.clone() on_click=Callback::new(move |_| actions.on_open.run(n_open.clone()))
            attr:data-testid="connection-card">
            <Stack gap="xs">
                <Group justify="between" gap="sm">
                    <Text bold=true mono=true>{conn.name.clone()}</Text>
                    <Pill color=pill_color>{pill_label}</Pill>
                </Group>
                <Group gap="sm">
                    <Text size="sm" mono=true>{conn.source.clone()}</Text>
                    <span class="weir-arr" aria-label="to">"──▶"</span>
                    <Text size="sm" mono=true>{conn.dest.clone()}</Text>
                </Group>
                <Text size="xs" dimmed=true mono=true>{format!("stream · {}", conn.stream)}</Text>
                <Group justify="between" gap="sm">
                    <span title=when_title><Text size="xs" dimmed=true mono=true>{when}</Text></span>
                    <Group gap="xs">
                        <Button variant="default" size="xs" bad=true stop_propagation=true
                            on_click=Callback::new(move |_| actions.on_delete.run(n_del.clone()))>"Delete"</Button>
                        {if resident {
                            view! {
                                <Button variant="default" size="xs" stop_propagation=true
                                    on_click=Callback::new(move |_| actions.on_stop.run(n_stop.clone()))>"Stop"</Button>
                                <Button size="xs" stop_propagation=true
                                    on_click=Callback::new(move |_| actions.on_start.run(n_start.clone()))>"Start"</Button>
                            }.into_any()
                        } else {
                            view! {
                                <Button size="xs" stop_propagation=true
                                    on_click=Callback::new(move |_| actions.on_run.run(n_run.clone()))>"Run"</Button>
                            }.into_any()
                        }}
                    </Group>
                </Group>
            </Stack>
        </Card>
    }
}
