//! Stream discovery for the connection form ([[WEIR-T-0218]]).
//!
//! Discovery reads the chosen source and its source-side config (the form fields with
//! the "Advanced JSON" merged over them). It re-runs when either changes: at once for a
//! new source, debounced while the config is being typed. Each run takes a ticket; an
//! answer that comes back after a newer run started is dropped.

use crate::components::SideConfig;
use crate::fetch::fetch_streams;
use crate::helpers::{side_config, SECRET_SENTINEL};
use leptos::prelude::*;

/// How long the config must stay still before discovery runs again.
pub(crate) const DEBOUNCE_MS: u32 = 500;

/// Request generations: each request takes a ticket, and only the latest ticket's
/// answer is applied.
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct Latest(u64);

impl Latest {
    /// Start a request; every older ticket is now stale.
    pub(crate) fn begin(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }

    pub(crate) fn is_current(&self, ticket: u64) -> bool {
        self.0 == ticket
    }
}

/// What the stream field says about discovery.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DiscoveryState {
    /// No source chosen.
    Idle,
    /// The source config is not complete yet (the reason); nothing was sent.
    Waiting(String),
    Loading,
    Found(usize),
    /// The server's reason.
    Failed(String),
}

impl DiscoveryState {
    /// The line shown under the stream field, and whether it is an error.
    pub(crate) fn message(&self) -> Option<(String, bool)> {
        match self {
            Self::Idle => None,
            Self::Waiting(why) => Some((
                format!("Streams are discovered once the source config is complete — {why}"),
                false,
            )),
            Self::Loading => Some(("Discovering streams…".to_string(), false)),
            Self::Found(0) => Some(("Discovery found no streams.".to_string(), false)),
            Self::Found(1) => Some(("1 stream discovered.".to_string(), false)),
            Self::Found(n) => Some((format!("{n} streams discovered."), false)),
            Self::Failed(why) => Some((format!("Stream discovery failed: {why}"), true)),
        }
    }
}

/// The source's discovered streams, what discovery is doing, and a manual re-run.
#[derive(Clone, Copy)]
pub(crate) struct Discovery {
    pub(crate) streams: RwSignal<Vec<String>>,
    pub(crate) state: RwSignal<DiscoveryState>,
    pub(crate) refresh: Callback<()>,
}

/// A loaded connection's stored secrets arrive as the sentinel ([[WEIR-T-0216]]); the
/// browser never has their values. Discovery runs without them: the keys that hold the
/// sentinel are dropped from the config sent, and named in a failure.
pub(crate) fn withhold_stored_secrets(
    config: serde_json::Value,
) -> (serde_json::Value, Vec<String>) {
    let serde_json::Value::Object(mut obj) = config else {
        return (config, Vec::new());
    };
    let withheld: Vec<String> = obj
        .iter()
        .filter(|(_, v)| v.as_str() == Some(SECRET_SENTINEL))
        .map(|(k, _)| k.clone())
        .collect();
    for k in &withheld {
        obj.remove(k);
    }
    (serde_json::Value::Object(obj), withheld)
}

/// What discovery reads: the source and its merged config (with the stored secrets it
/// withholds), once the source's contract has loaded (so a source that needs
/// credentials is not asked without them).
type Input = Option<(String, Result<(serde_json::Value, Vec<String>), String>)>;

impl Discovery {
    pub(crate) fn watch(src: RwSignal<String>, cfg: SideConfig) -> Self {
        let streams = RwSignal::new(Vec::<String>::new());
        let state = RwSignal::new(DiscoveryState::Idle);
        let latest = StoredValue::new(Latest::default());

        let input = Memo::new(move |_| -> Input {
            let s = src.get();
            if s.is_empty() || cfg.loaded.get() != s {
                return None;
            }
            let merged = cfg
                .props
                .with(|p| side_config("Source", p, &cfg.form.get(), &cfg.advanced.get()));
            Some((s, merged.map(withhold_stored_secrets)))
        });

        let run = move |input: Input, delay: u32| {
            let ticket = latest.try_update_value(Latest::begin).unwrap_or_default();
            let current = move || {
                latest
                    .try_with_value(|l| l.is_current(ticket))
                    .unwrap_or(false)
            };
            let (s, (config, withheld)) = match input {
                None => {
                    streams.set(Vec::new());
                    state.set(DiscoveryState::Idle);
                    return;
                }
                Some((_, Err(why))) => {
                    streams.set(Vec::new());
                    state.set(DiscoveryState::Waiting(why));
                    return;
                }
                Some((s, Ok(sent))) => (s, sent),
            };
            state.set(DiscoveryState::Loading);
            leptos::task::spawn_local(async move {
                if delay > 0 {
                    gloo_timers::future::TimeoutFuture::new(delay).await;
                    if !current() {
                        return;
                    }
                }
                let found = fetch_streams(&s, &config.to_string()).await;
                if !current() {
                    return;
                }
                match found {
                    Ok(names) => {
                        state.set(DiscoveryState::Found(names.len()));
                        streams.set(names);
                    }
                    Err(why) => {
                        let why = if withheld.is_empty() {
                            why
                        } else {
                            format!(
                                "{why} (stored secrets are not sent to discovery: retype {} to use it)",
                                withheld.join(", ")
                            )
                        };
                        streams.set(Vec::new());
                        state.set(DiscoveryState::Failed(why));
                    }
                }
            });
        };

        // A new source is discovered at once; edits to its config wait for typing to settle.
        Effect::new(move |prev: Option<String>| {
            let now = input.get();
            let s = now.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
            let delay = if prev.as_deref() == Some(s.as_str()) {
                DEBOUNCE_MS
            } else {
                0
            };
            run(now, delay);
            s
        });

        let refresh = Callback::new(move |_| run(input.get_untracked(), 0));
        Self {
            streams,
            state,
            refresh,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_answer_after_a_newer_request_is_stale() {
        let mut latest = Latest::default();
        let first = latest.begin();
        let second = latest.begin();
        // The first request's answer arrives last: it must be ignored.
        assert!(!latest.is_current(first));
        assert!(latest.is_current(second));
    }

    #[test]
    fn a_single_request_is_current_until_the_next() {
        let mut latest = Latest::default();
        let t = latest.begin();
        assert!(latest.is_current(t));
        latest.begin();
        assert!(!latest.is_current(t));
    }

    #[test]
    fn stored_secrets_are_withheld_from_discovery() {
        let cfg = serde_json::json!({ "host": "db", "token": SECRET_SENTINEL, "key": "new" });
        let (sent, withheld) = withhold_stored_secrets(cfg);
        assert_eq!(sent, serde_json::json!({ "host": "db", "key": "new" }));
        assert_eq!(withheld, vec!["token".to_string()]);
        let (sent, withheld) = withhold_stored_secrets(serde_json::json!({ "host": "db" }));
        assert_eq!(sent, serde_json::json!({ "host": "db" }));
        assert!(withheld.is_empty());
    }

    #[test]
    fn failures_are_errors_and_progress_is_not() {
        assert_eq!(DiscoveryState::Idle.message(), None);
        assert!(
            DiscoveryState::Failed("bad token".into())
                .message()
                .unwrap()
                .1
        );
        assert!(DiscoveryState::Failed("bad token".into())
            .message()
            .unwrap()
            .0
            .contains("bad token"));
        assert!(!DiscoveryState::Loading.message().unwrap().1);
        assert!(
            !DiscoveryState::Waiting("missing".into())
                .message()
                .unwrap()
                .1
        );
        assert_eq!(
            DiscoveryState::Found(2).message().unwrap().0,
            "2 streams discovered."
        );
    }
}
