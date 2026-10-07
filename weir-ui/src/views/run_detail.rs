//! One run's detail ([[WEIR-T-0219]]): `GET /runs/{id}` ([[WEIR-T-0189]]) plus the run's
//! connection state (committed cursor + chunks) and dead letters.
//!
//! The API scopes the cursor, chunk count, dead letters and logs to the connection,
//! not to one run, so the modal labels those sections as connection-level.

use crate::fetch::{get_fetch, get_json, Fetched};
use crate::helpers::{fmt_ts, log_color, run_duration, state_color};
use crate::models::{ConnState, DeadLetterRow, RunDetail};
use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use leptos::prelude::*;

/// What the modal holds for the selected run.
#[derive(Clone, PartialEq)]
enum Loaded {
    Wait,
    Failed(String),
    Ready(Box<RunDetail>),
}

/// A timestamp row value: absolute UTC time and "3m ago", or "—" when not set.
fn when(ms: Option<i64>) -> AnyView {
    match ms {
        Some(t) => view! {
            <span>{fmt_ts(t)}" · "<RelativeTime at=t as f64/></span>
        }
        .into_any(),
        None => view! { <span>"—"</span> }.into_any(),
    }
}

/// The run-detail modal. Set `run_id` and `open` to show one run; `on_connection`
/// opens the run's connection detail.
#[component]
pub(crate) fn RunDetailModal(
    open: RwSignal<bool>,
    run_id: RwSignal<Option<i64>>,
    on_connection: Callback<String>,
) -> impl IntoView {
    let loaded = RwSignal::new(Loaded::Wait);
    let state = RwSignal::new(ConnState::default());
    let dls = RwSignal::new(Vec::<DeadLetterRow>::new());
    let reload = RwSignal::new(0u32);

    Effect::new(move |_| {
        reload.track();
        let (true, Some(id)) = (open.get(), run_id.get()) else {
            return;
        };
        loaded.set(Loaded::Wait);
        state.set(ConnState::default());
        dls.set(Vec::new());
        leptos::task::spawn_local(async move {
            let got = get_fetch::<RunDetail>(format!("/runs/{id}")).await;
            // A later selection wins over a slow answer for an earlier one.
            if run_id.get_untracked() != Some(id) {
                return;
            }
            let run = match got {
                Fetched::Ok(r) => r,
                Fetched::Failed(s, m) => {
                    return loaded.set(Loaded::Failed(format!("{m} · HTTP {s}")))
                }
                Fetched::Unauthorized => return loaded.set(Loaded::Failed("not signed in".into())),
                Fetched::Network => return loaded.set(Loaded::Failed("server unreachable".into())),
            };
            let conn = run.connection.clone();
            loaded.set(Loaded::Ready(Box::new(run)));
            let st = get_json::<ConnState>(format!("/connections/{conn}/state")).await;
            let dl =
                get_json::<Vec<DeadLetterRow>>(format!("/connections/{conn}/dead-letters")).await;
            if run_id.get_untracked() == Some(id) {
                state.set(st);
                dls.set(dl);
            }
        });
    });

    let title = Signal::derive(move || match run_id.get() {
        Some(id) => format!("Run #{id}"),
        None => "Run".to_string(),
    });

    view! {
        <Modal open=open title=title size="lg">
            {move || match loaded.get() {
                Loaded::Wait => view! { <Loading label="Loading run…"/> }.into_any(),
                Loaded::Failed(e) => view! {
                    <Alert title="Couldn't load the run" color=token::BAD><Text size="sm">{e}</Text></Alert>
                }.into_any(),
                Loaded::Ready(r) => run_body(*r, state, dls, reload, open, on_connection),
            }}
        </Modal>
    }
}

fn run_body(
    r: RunDetail,
    state: RwSignal<ConnState>,
    dls: RwSignal<Vec<DeadLetterRow>>,
    reload: RwSignal<u32>,
    open: RwSignal<bool>,
    on_connection: Callback<String>,
) -> AnyView {
    let conn = r.connection.clone();
    let duration = run_duration(r.duration_ms, &r.state);
    let error = r.error.clone().filter(|e| !e.is_empty());
    let log_lines: Vec<LogLine> = r
        .logs
        .iter()
        .map(|l| LogLine::new(l.message.clone()).level_color(l.level.clone(), log_color(&l.level)))
        .collect();
    let state_pill = state_color(&r.state);
    let (status, pill_text) = (r.state.clone(), r.state.clone());
    let (connection, stream) = (r.connection.clone(), format!("· {}", r.stream));
    let (attempt, rows, dead) = (r.attempt, r.rows_written, r.dead_lettered);
    let (started, ended) = (when(r.started_at), when(r.finished_at));
    view! {
        <Stack gap="sm">
            <Group gap="sm">
                <Pill color=state_pill>{pill_text}</Pill>
                <Text size="sm" bold=true mono=true>{connection}</Text>
                <Text size="sm" dimmed=true mono=true>{stream}</Text>
                <Button variant="default" size="xs" on_click=Callback::new(move |_| reload.update(|n| *n += 1))>
                    "Refresh"
                </Button>
                <Button variant="default" size="xs" on_click=Callback::new(move |_| {
                    open.set(false);
                    on_connection.run(conn.clone());
                })>
                    "Open connection"
                </Button>
            </Group>
            {error.map(|e| view! {
                <Alert title="Run error" color=token::BAD><Text size="sm">{e}</Text></Alert>
            })}
            <DetailList mono=true>
                <KeyValue label="status">{status}</KeyValue>
                <KeyValue label="attempt">{attempt}</KeyValue>
                <KeyValue label="started">{started}</KeyValue>
                <KeyValue label="ended">{ended}</KeyValue>
                <KeyValue label="duration">{duration}</KeyValue>
                <KeyValue label="rows written">{rows}</KeyValue>
                <KeyValue label="dead-lettered">{dead}</KeyValue>
            </DetailList>
        </Stack>
        <SectionLabel label="Committed state · connection" divider=true/>
        <Text size="xs" dimmed=true>"The API keeps the cursor and chunk count per connection, not per run."</Text>
        {move || {
            let st = state.get();
            view! {
                <DetailList mono=true>
                    <KeyValue label="committed cursor">{st.cursor.unwrap_or_else(|| "—".to_string())}</KeyValue>
                    <KeyValue label="committed chunks">{st.chunks}</KeyValue>
                </DetailList>
            }
        }}
        <SectionLabel label="Dead-letters · connection" count=Signal::derive(move || Some(dls.get().len())) divider=true/>
        {move || {
            let rows = dls.get();
            if rows.is_empty() {
                view! { <Text size="sm" dimmed=true>"None."</Text> }.into_any()
            } else {
                view! {
                    <Table mono=true fixed=true widths=vec!["38%".into(), "62%".into()] label="Dead-letters">
                        <thead><tr><th>"reason"</th><th>"record"</th></tr></thead>
                        <tbody>
                            {rows.into_iter().map(|d| view! {
                                <tr><td title=d.reason.clone()>{d.reason.clone()}</td><td title=d.record.clone()>{d.record.clone()}</td></tr>
                            }).collect_view()}
                        </tbody>
                    </Table>
                }.into_any()
            }
        }}
        <SectionLabel label="Logs · connection" divider=true/>
        <LogView label="Run logs" empty="No logs." max_height="260px"
            lines=log_lines/>
    }
    .into_any()
}
