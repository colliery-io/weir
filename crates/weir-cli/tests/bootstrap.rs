//! [[WEIR-T-0164]]: `weir serve` (and `weir api`, same helper) mints + prints the
//! bootstrap admin key on a fresh store exactly once; a restart on the same store
//! neither re-mints nor re-prints.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn spawn_serve(db: &str) -> (Child, mpsc::Receiver<String>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_weir"))
        .args(["--db", db, "serve", "--poll", "0.05"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn weir serve");
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    (child, rx)
}

/// Collect stdout lines until `pred` matches one, the window elapses, or the pipe closes.
/// Returns (lines, matched).
fn collect_until(
    rx: &mpsc::Receiver<String>,
    window: Duration,
    pred: impl Fn(&str) -> bool,
) -> (Vec<String>, bool) {
    let deadline = Instant::now() + window;
    let mut lines = Vec::new();
    loop {
        let now = Instant::now();
        if now >= deadline {
            return (lines, false);
        }
        match rx.recv_timeout(deadline - now) {
            Ok(line) => {
                let hit = pred(&line);
                lines.push(line);
                if hit {
                    return (lines, true);
                }
            }
            Err(_) => return (lines, false),
        }
    }
}

fn stop(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn serve_mints_bootstrap_key_once_then_never_again() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("weir.db");
    let db = db.to_str().unwrap();

    // Fresh store: the minted admin key is printed before the serving banner.
    let (child, rx) = collect_fresh(db);
    stop(child);
    let (lines, saw_key) = rx;
    assert!(
        saw_key,
        "fresh `weir serve` should print the minted admin key; got: {lines:?}"
    );

    // Restart on the same store: startup reaches the serving banner with no key line.
    let (child, rx) = spawn_serve(db);
    let (lines, saw_banner) =
        collect_until(&rx, Duration::from_secs(30), |l| l.contains("weir serving"));
    stop(child);
    assert!(
        saw_banner,
        "restarted serve should reach its banner; got: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("weirk_")),
        "restart must not re-mint or re-print a key; got: {lines:?}"
    );
}

/// Fresh-store spawn: wait for the key line (it precedes the serving banner).
fn collect_fresh(db: &str) -> (Child, (Vec<String>, bool)) {
    let (child, rx) = spawn_serve(db);
    let out = collect_until(&rx, Duration::from_secs(30), |l| l.contains("weirk_"));
    (child, out)
}
