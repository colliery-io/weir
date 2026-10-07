//! Health: per-connection freshness, errors, dead letters and throughput.

use crate::helpers::{fmt_lag, health_color, health_rank};
use crate::models::ConnHealth;
use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use leptos::prelude::*;

pub(crate) fn health_view(health: RwSignal<Vec<ConnHealth>>, open_detail: Callback<String>) -> AnyView {
    view! {
        <Panel title="Needs attention" caption="amber + red connections, worst first">
            {move || {
                let mut hs = health.get();
                hs.sort_by_key(|h| health_rank(&h.status));
                let attn: Vec<ConnHealth> = hs.into_iter()
                    .filter(|h| h.status == "red" || h.status == "amber").collect();
                if attn.is_empty() {
                    view! { <Text size="sm" dimmed=true>"All connections healthy."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true label="Needs attention">
                            <thead><tr><th>"status"</th><th>"connection"</th><th class="cl-num">"lag"</th><th class="cl-num">"errors"</th><th class="cl-num">"dead"</th></tr></thead>
                            <tbody>
                                {attn.into_iter().map(|h| {
                                    let name = h.connection.clone();
                                    view! {
                                        <TableRow label=h.connection.clone() on_click=Callback::new(move |_| open_detail.run(name.clone()))>
                                            <td><Pill color=health_color(&h.status)>{h.status.clone()}</Pill></td>
                                            <td>{h.connection.clone()}</td>
                                            <td class="cl-num">{fmt_lag(h.lag_ms)}</td>
                                            <td class="cl-num">{format!("{:.0}%", h.error_rate * 100.0)}</td>
                                            <td class="cl-num">{h.dead_letters.to_string()}</td>
                                        </TableRow>
                                    }
                                }).collect_view()}
                            </tbody>
                        </Table>
                    }.into_any()
                }
            }}
        </Panel>
        <Panel title="Connection health" caption="freshness · errors · dead letters · throughput">
            {move || {
                let mut hs = health.get();
                hs.sort_by_key(|h| health_rank(&h.status));
                if hs.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No connections yet — add one in Setup."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=3>
                            {hs.into_iter().map(|h| {
                                let name = h.connection.clone();
                                let spark: Vec<f64> = h.throughput.iter().map(|v| *v as f64).collect();
                                let spark_label = format!("{} throughput", h.connection);
                                view! {
                                    <Card label=h.connection.clone() on_click=Callback::new(move |_| open_detail.run(name.clone()))
                                        attr:data-testid="health-card">
                                        <Stack gap="xs">
                                            <Group gap="sm">
                                                <Pill color=health_color(&h.status)>{h.status.clone()}</Pill>
                                                <Text bold=true mono=true>{h.connection.clone()}</Text>
                                            </Group>
                                            <Group gap="sm" wrap=true>
                                                <Text size="xs" dimmed=true mono=true>{format!("lag {}", fmt_lag(h.lag_ms))}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("err {:.0}%", h.error_rate * 100.0)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("dl {}", h.dead_letters)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("{} rows", h.rows_recent)}</Text>
                                            </Group>
                                            {(!spark.is_empty()).then(|| view! {
                                                <Sparkline values=spark fluid=true height=22.0 color=token::ICE
                                                    label=spark_label.clone()/>
                                            })}
                                        </Stack>
                                    </Card>
                                }
                            }).collect_view()}
                        </SimpleGrid>
                    }.into_any()
                }
            }}
        </Panel>
    }
    .into_any()
}
