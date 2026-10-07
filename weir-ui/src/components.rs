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

/// One side (source or destination) of the connection form's config ([[WEIR-T-0214]]):
/// the connector's contract, the values its fields hold, and an "Advanced JSON"
/// override that is merged over them on save.
#[derive(Clone, Copy)]
pub(crate) struct SideConfig {
    pub(crate) props: RwSignal<Vec<Prop>>,
    /// The form values, as a JSON object string.
    pub(crate) form: RwSignal<String>,
    pub(crate) advanced: RwSignal<String>,
    pub(crate) show_advanced: RwSignal<bool>,
}

impl SideConfig {
    pub(crate) fn new() -> Self {
        Self {
            props: RwSignal::new(Vec::new()),
            form: RwSignal::new("{}".to_string()),
            advanced: RwSignal::new(String::new()),
            show_advanced: RwSignal::new(false),
        }
    }

    /// A different connector: its contract replaces the old one, and the old values go.
    pub(crate) fn reset(&self) {
        self.form.set("{}".to_string());
        self.advanced.set(String::new());
    }
}

/// One field of a connector's config contract, bound to the side's form JSON.
/// String and number fields are text inputs, secret fields password inputs, `enum`
/// fields selects, and booleans switches. A required field has a `*` on its label.
pub(crate) fn config_field(p: Prop, form: RwSignal<String>) -> AnyView {
    let label = if p.required { format!("{} *", p.key) } else { p.key.clone() };
    let (key, kind) = (p.key.clone(), p.kind.clone());
    let set = move |v: &str| form.set(cfg_set(&form.get_untracked(), &key, v, &kind));
    let now = cfg_get(&form.get_untracked(), &p.key);
    if p.kind == "boolean" {
        let checked = RwSignal::new(now == "true");
        let on_change = Callback::new(move |on: bool| set(if on { "true" } else { "false" }));
        return view! { <Switch checked=checked label=label on_change=on_change/> }.into_any();
    }
    let field = RwSignal::new(now);
    let on_input = Callback::new(move |v: String| set(&v));
    if !p.options.is_empty() {
        return view! {
            <Select label=label placeholder="—" options=p.options value=field required=p.required on_change=on_input/>
        }
        .into_any();
    }
    if p.secret {
        return view! {
            <PasswordInput label=label value=field required=p.required on_input=on_input autocomplete="new-password"/>
        }
        .into_any();
    }
    view! {
        <TextInput label=label placeholder=p.kind value=field required=p.required on_input=on_input mono=true/>
    }
    .into_any()
}

/// One side of the connection form: a field per contract property, then a switch
/// that shows the "Advanced JSON" override.
pub(crate) fn config_side(side: &'static str, connector: RwSignal<String>, cfg: SideConfig) -> impl IntoView {
    let caption = move || match connector.get().as_str() {
        "" => format!("config · {}", side.to_lowercase()),
        c => format!("config · {} · {c}", side.to_lowercase()),
    };
    let testid = format!("config-{}", side.to_lowercase());
    view! {
        <div class="weir-form" data-testid=testid>
            {move || view! { <SectionLabel label=caption() divider=true/> }}
            // Its own reactive scope: a parent component renders its children untracked,
            // so the fields follow the contract here, not through a re-render of the view.
            {move || {
                let props = cfg.props.get();
                if props.is_empty() {
                    view! { <Text size="xs" dimmed=true>"No config contract: use Advanced JSON."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=2>
                            {props.into_iter().map(|p| config_field(p, cfg.form)).collect_view()}
                        </SimpleGrid>
                    }.into_any()
                }
            }}
            <Switch checked=cfg.show_advanced label=format!("{side} advanced JSON")/>
            {move || cfg.show_advanced.get().then(|| view! {
                <Textarea label=format!("{side} advanced JSON · merged over the fields") rows=3 mono=true
                    placeholder="{}" value=cfg.advanced/>
            })}
        </div>
    }
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
