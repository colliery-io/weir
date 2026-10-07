//! The four top-level views the shell routes between, and the run-detail modal.

mod health;
mod operations;
mod platform;
mod run_detail;
mod setup;

pub(crate) use health::health_view;
pub(crate) use operations::{operations_view, RunFeed};
pub(crate) use platform::platform_view;
pub(crate) use run_detail::RunDetailModal;
pub(crate) use setup::{setup_view, SetupState, SyncForm};
