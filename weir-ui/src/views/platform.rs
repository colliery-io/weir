//! Platform: the cross-tenant rollup for platform admins.

use crate::helpers::{health_color, health_rank};
use crate::models::PlatformHealth;
use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use leptos::prelude::*;

pub(crate) fn platform_view(platform: RwSignal<PlatformHealth>, drill_tenant: Callback<String>) -> AnyView {
    view! {
        <SimpleGrid cols=3>
            <StatTile label="Tenants" value=Signal::derive(move || platform.get().tenants.len().to_string())/>
            <StatTile label="Active" value=Signal::derive(move || platform.get().active_tenants.to_string()) color=token::OK/>
            <StatTile label="Queue depth" value=Signal::derive(move || platform.get().total_queue_depth.to_string()) sub="runs waiting, all tenants"/>
        </SimpleGrid>
        <Panel title="Tenant health" caption="worst first · click to drill in">
            {move || {
                let mut ts = platform.get().tenants;
                ts.sort_by_key(|t| health_rank(&t.status));
                if ts.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No tenants."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=3>
                            {ts.into_iter().map(|t| {
                                let tid = t.tenant.clone();
                                view! {
                                    <Card label=t.tenant.clone() on_click=Callback::new(move |_| drill_tenant.run(tid.clone()))
                                        attr:data-testid="tenant-card">
                                        <Stack gap="xs">
                                            <Group gap="sm">
                                                <Pill color=health_color(&t.status)>{t.status.clone()}</Pill>
                                                <Text bold=true mono=true>{t.tenant.clone()}</Text>
                                            </Group>
                                            <Group gap="sm" wrap=true>
                                                <Text size="xs" dimmed=true mono=true>{format!("{} conns", t.connections)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("{} failing", t.needs_attention)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("dl {}", t.dead_letters)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("queue {}", t.queue_depth)}</Text>
                                            </Group>
                                        </Stack>
                                    </Card>
                                }
                            }).collect_view()}
                        </SimpleGrid>
                    }.into_any()
                }
            }}
        </Panel>
        <Panel title="Needs attention" caption="failing connections across all tenants">
            {move || {
                let attn = platform.get().needs_attention;
                if attn.is_empty() {
                    view! { <Text size="sm" dimmed=true>"All tenants healthy."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true label="Needs attention">
                            <thead><tr><th>"status"</th><th>"tenant"</th><th>"connection"</th></tr></thead>
                            <tbody>
                                {attn.into_iter().map(|a| {
                                    let tid = a.tenant.clone();
                                    view! {
                                        <TableRow label=format!("{} · {}", a.tenant, a.connection)
                                            on_click=Callback::new(move |_| drill_tenant.run(tid.clone()))>
                                            <td><Pill color=health_color(&a.status)>{a.status.clone()}</Pill></td>
                                            <td>{a.tenant.clone()}</td>
                                            <td>{a.connection.clone()}</td>
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
