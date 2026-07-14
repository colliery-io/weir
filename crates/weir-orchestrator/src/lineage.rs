//! OpenLineage emission ([[WEIR-A-0022]] / [[WEIR-T-0100]]) — weir's runs as OpenLineage `RunEvent`s
//! (START / COMPLETE / FAIL), **async + best-effort** (a transport failure never fails the run), to a
//! pluggable transport: an HTTP endpoint (Marquez/DataHub), the console, or off (default). No SDK —
//! the event is a spec-shaped JSON object.

use crate::{ConnectorRef, WorkSpec};
use serde_json::{Value, json};

/// `producer` URI stamped on every event + facet.
const PRODUCER: &str = "https://github.com/colliery-io/weir";

#[derive(Clone, Debug)]
enum Transport {
    Off,
    Console,
    Http(String),
}

/// The lineage emitter. Cheap to clone + construct (`from_env`); held by the [`crate::Worker`].
#[derive(Clone, Debug)]
pub struct Lineage {
    transport: Transport,
}

impl Lineage {
    /// Read the transport from `WEIR_OPENLINEAGE_URL`: `console` → stderr, an `http(s)://…` URL → POST,
    /// unset/empty → off.
    pub fn from_env() -> Lineage {
        let transport = match std::env::var("WEIR_OPENLINEAGE_URL").ok() {
            Some(u) if u.eq_ignore_ascii_case("console") => Transport::Console,
            Some(u) if !u.trim().is_empty() => Transport::Http(u),
            _ => Transport::Off,
        };
        Lineage { transport }
    }

    pub fn enabled(&self) -> bool {
        !matches!(self.transport, Transport::Off)
    }

    pub fn emit_start(&self, spec: &WorkSpec, work_unit_id: i64) {
        self.emit("START", spec, work_unit_id, None);
    }
    pub fn emit_complete(&self, spec: &WorkSpec, work_unit_id: i64, rows: i64) {
        self.emit("COMPLETE", spec, work_unit_id, Some(rows));
    }
    pub fn emit_fail(&self, spec: &WorkSpec, work_unit_id: i64) {
        self.emit("FAIL", spec, work_unit_id, None);
    }

    fn emit(&self, event_type: &str, spec: &WorkSpec, work_unit_id: i64, rows: Option<i64>) {
        if matches!(self.transport, Transport::Off) {
            return;
        }
        let event = run_event(event_type, spec, work_unit_id, rows);
        match &self.transport {
            Transport::Console => eprintln!("openlineage {event}"),
            Transport::Http(url) => {
                // Fire-and-forget: never block or fail the run on a transport error.
                let url = url.clone();
                tokio::spawn(async move {
                    let _ = reqwest::Client::new().post(&url).json(&event).send().await;
                });
            }
            Transport::Off => {}
        }
    }
}

fn ref_namespace(r: &ConnectorRef) -> String {
    match r {
        ConnectorRef::Wasm { package, .. } => package.clone(),
    }
}

fn dataset(r: &ConnectorRef, stream: &str, rows: Option<i64>) -> Value {
    let mut d = json!({ "namespace": ref_namespace(r), "name": stream });
    if let Some(n) = rows {
        d["facets"] = json!({
            "outputStatistics": {
                "_producer": PRODUCER,
                "_schemaURL": "https://openlineage.io/spec/facets/1-0-0/OutputStatisticsOutputDatasetFacet.json",
                "rowCount": n
            }
        });
    }
    d
}

/// A deterministic UUID-shaped runId from the (monotonic) work-unit id, so START + COMPLETE of one
/// run share a runId without a uuid dep.
fn run_uuid(id: i64) -> String {
    let u = id as u64;
    format!(
        "00000000-0000-0000-{:04x}-{:012x}",
        (u >> 48) & 0xffff,
        u & 0xffff_ffff_ffff
    )
}

/// Build a spec-shaped OpenLineage `RunEvent`.
pub fn run_event(event_type: &str, spec: &WorkSpec, work_unit_id: i64, rows: Option<i64>) -> Value {
    json!({
        "eventType": event_type,
        "eventTime": chrono::Utc::now().to_rfc3339(),
        "run": { "runId": run_uuid(work_unit_id) },
        "job": { "namespace": "weir", "name": format!("{}/{}", spec.tenant, spec.connection) },
        "inputs": [ dataset(&spec.source, &spec.stream.stream, None) ],
        "outputs": [ dataset(&spec.dest, &spec.stream.stream, rows) ],
        "producer": PRODUCER,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};

    fn spec() -> WorkSpec {
        WorkSpec {
            connection: "fx".to_string(),
            tenant: "acme".to_string(),
            stream: ConfiguredStream {
                stream: "rates".to_string(),
                sync_mode: SyncMode::FullRefresh,
                cursor_field: None,
                primary_key: None,
                write_mode: WriteMode::Append,
                mapping: MappingSpec::default(),
            },
            source: ConnectorRef::Wasm {
                search_path: "c".to_string(),
                package: "weir-frankfurter-pkg".to_string(),
                version: "0".to_string(),
                origin: crate::Origin::FirstParty,
            },
            dest: ConnectorRef::Wasm {
                search_path: "c".to_string(),
                package: "weir-arrow-sink-pkg".to_string(),
                version: "0".to_string(),
                origin: crate::Origin::FirstParty,
            },
            source_config: Config {
                json: "{}".to_string(),
            },
            dest_config: Config {
                json: "{}".to_string(),
            },
            state_key: None,
            seed_cursor: None,
            partition: None,
            execution_mode: Default::default(),
        }
    }

    #[test]
    fn start_and_complete_are_valid_openlineage() {
        let s = spec();
        let start = run_event("START", &s, 42, None);
        assert_eq!(start["eventType"], "START");
        assert_eq!(start["job"]["namespace"], "weir");
        assert_eq!(start["job"]["name"], "acme/fx"); // tenant/connection
        assert_eq!(start["inputs"][0]["namespace"], "weir-frankfurter-pkg");
        assert_eq!(start["inputs"][0]["name"], "rates");
        assert_eq!(start["outputs"][0]["namespace"], "weir-arrow-sink-pkg");
        // runId is a UUID shape.
        assert_eq!(start["run"]["runId"].as_str().unwrap().len(), 36);

        let done = run_event("COMPLETE", &s, 42, Some(1234));
        assert_eq!(done["eventType"], "COMPLETE");
        // Same run → same runId.
        assert_eq!(done["run"]["runId"], start["run"]["runId"]);
        assert_eq!(
            done["outputs"][0]["facets"]["outputStatistics"]["rowCount"],
            1234
        );
    }
}
