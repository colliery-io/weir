//! The connector error taxonomy (WEIR-A-0014 §5).
//!
//! Four kinds, each with a defined Engine behaviour:
//! - [`ErrorKind::Config`]    — surface to the user; do **not** retry.
//! - [`ErrorKind::Transient`] — the Engine retries with backoff.
//! - [`ErrorKind::Record`]    — dead-letter the offending record; continue.
//! - [`ErrorKind::Fatal`]     — abort the run.

use serde::{Deserialize, Serialize};

/// How the Sync Engine should react to a [`ConnectorError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum ErrorKind {
    /// Bad/insufficient configuration. Surface to the user; do not retry.
    Config,
    /// A transient failure (network, rate limit, 5xx). The Engine retries with backoff.
    Transient,
    /// A single record could not be processed. Dead-letter it and continue.
    /// (Named `RecordLevel`, not `Record`, because `record` is a reserved WIT keyword.)
    RecordLevel,
    /// Unrecoverable; abort the run.
    Fatal,
}

/// One structured-context entry on a [`ConnectorError`]. A `Vec` of these is
/// used instead of a map so the type projects cleanly to WIT (records + list).
/// (Named `ContextPair`, not `ErrorContext`, because `error-context` is a
/// reserved built-in in the WIT component model.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ContextPair {
    pub key: String,
    pub value: String,
}

/// A structured error returned across the connector boundary.
///
/// Travels in the typed return of `discover` (inside [`crate::DiscoverOutcome`])
/// and inside the raw envelopes of `read`/`write` (as the `Err` arm).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
#[error("{kind:?}: {message}")]
pub struct ConnectorError {
    /// The taxonomy bucket that determines Engine behaviour.
    pub kind: ErrorKind,
    /// Human-readable description.
    pub message: String,
    /// Whether a retry could plausibly succeed. The Engine treats
    /// [`ErrorKind::Transient`] as retryable by default; this allows a
    /// connector to override per-error.
    pub retryable: bool,
    /// Free-form structured context (endpoint, status code, stream, …).
    pub context: Vec<ContextPair>,
}

impl ConnectorError {
    /// Construct an error of `kind` with `message`. `retryable` defaults to
    /// `true` only for [`ErrorKind::Transient`].
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        let retryable = matches!(kind, ErrorKind::Transient);
        Self {
            kind,
            message: message.into(),
            retryable,
            context: Vec::new(),
        }
    }

    /// Convenience: a config error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Config, message)
    }

    /// Convenience: a transient (retryable) error.
    pub fn transient(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Transient, message)
    }

    /// Convenience: a fatal error.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Fatal, message)
    }

    /// Attach a context key/value.
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push(ContextPair {
            key: key.into(),
            value: value.into(),
        });
        self
    }
}
