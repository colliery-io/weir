//! Operations: connection cards and the live run feed.

use crate::components::{ConnActions, ConnectionCard};
use crate::helpers::{latest_run, run_metrics, state_color};
use crate::models::{Connection, RunRow};
use aurora_leptos::components::*;
use leptos::prelude::*;

pub(crate) fn operations_view(
    connections: RwSignal<Vec<Connection>>,
    runs: RwSignal<Vec<RunRow>>,
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
        <Panel title="Run feed" caption="most recent first">
            {move || {
                let rs = runs.get();
                if rs.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No runs yet."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true fixed=true widths=vec!["190px".into(), "22%".into(), "110px".into(), "auto".into()]
                            label="Run feed" min_width="680px">
                            <thead><tr><th>"#"</th><th>"connection"</th><th>"state"</th><th>"detail"</th></tr></thead>
                            <tbody>
                                {rs.into_iter().map(|r| {
                                    let name = r.connection.clone();
                                    let metrics = run_metrics(&r);
                                    view! {
                                        <TableRow label=format!("run {} · {}", r.id, r.connection)
                                            on_click=Callback::new(move |_| actions.on_open.run(name.clone()))>
                                            <td>{r.id}</td>
                                            <td>{r.connection.clone()}</td>
                                            <td><Pill color=state_color(&r.state)>{r.state.clone()}</Pill></td>
                                            <td title=metrics.clone()>{metrics.clone()}</td>
                                        </TableRow>
                                    }
                                }).collect_view()}
                            </tbody>
                        </Table>
                    }.into_any()
                }
            }}
        </Panel>
    }
    .into_any()
}
