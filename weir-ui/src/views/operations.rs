//! Operations: connection cards and the live run feed.

use crate::components::{ConnActions, ConnectionCard};
use crate::fetch::{get_fetch, Fetched};
use crate::helpers::{
    feed_cursor, fmt_ts, latest_run, merge_feed, parse_rfc3339_ms, run_duration, run_metrics,
    state_color, FEED_PAGE,
};
use crate::models::{Connection, RunRow};
use aurora_leptos::components::*;
use aurora_leptos::data::{format_relative, use_now};
use leptos::prelude::*;

/// The feed's start column ([[WEIR-T-0224]]): "3m ago" (ticking with the shared clock),
/// with the UTC time as the tooltip; "—" until the run starts.
fn started_cell(started_at: Option<&str>) -> AnyView {
    match started_at.and_then(parse_rfc3339_ms) {
        Some(ms) => {
            let now = use_now();
            view! {
                <td data-testid="run-started" title=fmt_ts(ms)>
                    {move || format_relative(now.get() - ms as f64)}
                </td>
            }
            .into_any()
        }
        None => view! { <td data-testid="run-started">"—"</td> }.into_any(),
    }
}

/// The run feed's paging state ([[WEIR-T-0219]]). The live first page is the polled
/// `runs` signal; `older` holds the pages loaded with `GET /runs?before=…`
/// ([[WEIR-T-0189]]). Lives in the shell so it survives a switch of view.
#[derive(Clone, Copy)]
pub(crate) struct RunFeed {
    runs: RwSignal<Vec<RunRow>>,
    older: RwSignal<Vec<RunRow>>,
    /// False once an older page came back short: the history has no more runs.
    more: RwSignal<bool>,
    loading: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    /// Opens one run's detail (by id).
    pub(crate) on_open_run: Callback<i64>,
}

impl RunFeed {
    pub(crate) fn new(runs: RwSignal<Vec<RunRow>>, on_open_run: Callback<i64>) -> Self {
        Self {
            runs,
            older: RwSignal::new(Vec::new()),
            more: RwSignal::new(true),
            loading: RwSignal::new(false),
            error: RwSignal::new(None),
            on_open_run,
        }
    }

    /// Put a fresh live first page in place. Once older pages are loaded, the rows that
    /// age out of the live page move to `older`, so no run falls into a gap.
    pub(crate) fn set_live(&self, live: Vec<RunRow>) {
        if !self.older.get_untracked().is_empty() {
            let prev = self.runs.get_untracked();
            self.older.update(|o| *o = merge_feed(&prev, o));
        }
        self.runs.set(live);
    }

    /// The rows the feed shows: live over older, newest first.
    fn rows(&self) -> Vec<RunRow> {
        merge_feed(&self.runs.get(), &self.older.get())
    }

    /// Fetch the next older page (`before` = the smallest id shown).
    fn load_more(self) {
        let Some(before) = feed_cursor(&merge_feed(
            &self.runs.get_untracked(),
            &self.older.get_untracked(),
        )) else {
            return;
        };
        self.loading.set(true);
        leptos::task::spawn_local(async move {
            let url = format!("/runs?limit={FEED_PAGE}&before={before}");
            match get_fetch::<Vec<RunRow>>(url).await {
                Fetched::Ok(page) => {
                    self.more.set(page.len() >= FEED_PAGE);
                    self.older.update(|o| *o = merge_feed(&page, o));
                    self.error.set(None);
                }
                Fetched::Failed(s, m) => self.error.set(Some(format!("{m} · HTTP {s}"))),
                Fetched::Unauthorized => self.error.set(Some("not signed in".into())),
                Fetched::Network => self.error.set(Some("server unreachable".into())),
            }
            self.loading.set(false);
        });
    }
}

pub(crate) fn operations_view(
    connections: RwSignal<Vec<Connection>>,
    runs: RwSignal<Vec<RunRow>>,
    feed: RunFeed,
    actions: ConnActions,
) -> AnyView {
    view! {
        <Panel title="Connections" caption="live runs">
            {move || {
                let rs = runs.get();
                let cs = connections.get();
                if cs.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No connections yet — add one in Setup."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=3>
                            {cs.into_iter().map(|c| {
                                let last = latest_run(&rs, &c.name);
                                view! { <ConnectionCard conn=c last=last actions=actions/> }
                            }).collect_view()}
                        </SimpleGrid>
                    }.into_any()
                }
            }}
        </Panel>
        <Panel title="Run feed" caption="most recent first · click a run for its detail">
            {move || {
                let rs = feed.rows();
                if rs.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No runs yet."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true fixed=true
                            widths=vec!["190px".into(), "20%".into(), "110px".into(), "110px".into(), "100px".into(), "auto".into()]
                            label="Run feed" min_width="890px">
                            <thead><tr><th>"#"</th><th>"connection"</th><th>"state"</th><th>"started"</th><th>"duration"</th><th>"detail"</th></tr></thead>
                            <tbody>
                                {rs.into_iter().map(|r| {
                                    let id = r.id;
                                    let metrics = run_metrics(&r);
                                    let duration = run_duration(r.duration_ms, &r.state);
                                    view! {
                                        <TableRow label=format!("run {} · {}", r.id, r.connection)
                                            on_click=Callback::new(move |_| feed.on_open_run.run(id))>
                                            <td>{r.id}</td>
                                            <td>{r.connection.clone()}</td>
                                            <td><Pill color=state_color(&r.state)>{r.state.clone()}</Pill></td>
                                            {started_cell(r.started_at.as_deref())}
                                            <td>{duration}</td>
                                            <td title=metrics.clone()>{metrics.clone()}</td>
                                        </TableRow>
                                    }
                                }).collect_view()}
                            </tbody>
                        </Table>
                    }.into_any()
                }
            }}
            {move || feed.error.get().map(|e| view! { <Text size="sm" dimmed=true>{format!("Couldn't load older runs: {e}")}</Text> })}
            {move || {
                let has_rows = !feed.runs.get().is_empty();
                if has_rows && feed.more.get() {
                    view! {
                        <Group gap="sm">
                            <Button variant="default" size="xs" loading=feed.loading loading_label="Loading…"
                                on_click=Callback::new(move |_| feed.load_more())>
                                "Load older runs"
                            </Button>
                        </Group>
                    }.into_any()
                } else if has_rows {
                    view! { <Text size="xs" dimmed=true>"No older runs."</Text> }.into_any()
                } else {
                    ().into_any()
                }
            }}
        </Panel>
    }
    .into_any()
}
