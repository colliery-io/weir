//! weir control-plane UI — Leptos (CSR) on the Colliery Aurora design system
//! ([[WEIR-A-0035]]), light and dark ([[WEIR-T-0191]]). Aurora ships the chrome;
//! weir supplies the data + vocabulary.
//! Shell + Operations + Setup ([[WEIR-T-0079]]/[[WEIR-T-0080]]/[[WEIR-T-0081]]).
//!
//! Module map: `app` (shell, state, routing, modals), `views` (operations, health,
//! platform, setup), `components` (shared widgets), `fetch` (HTTP + auth + tenant
//! scoping), `models` (DTOs), `helpers` (pure formatting).

mod app;
mod components;
mod fetch;
mod helpers;
mod models;
mod views;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
