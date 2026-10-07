//! The four top-level views the shell routes between.

mod health;
mod operations;
mod platform;
mod setup;

pub(crate) use health::health_view;
pub(crate) use operations::operations_view;
pub(crate) use platform::platform_view;
pub(crate) use setup::{setup_view, SetupState};
