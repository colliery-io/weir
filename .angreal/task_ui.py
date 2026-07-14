"""weir web UI: build the embedded Leptos app + a dev demo harness.

`angreal ui build`  — trunk-build the UI (embeds into the `weir` binary on next cargo build).
`angreal ui demo`   — build UI + weir, seed self-contained demo connections, serve the
                      control plane, and drive runs on a loop so the live feed shows every
                      state (success / running / dead-letters / failed). Ctrl-C to stop.

The demo uses only self-contained connectors (Echo/Slow/Faulty → ArrowSink), so it needs no
external API or database — point a browser at the printed URL and watch it move.
"""

import json
import os
import shutil
import subprocess
import tempfile
import time
import urllib.request

import angreal

root = os.path.join(angreal.get_root(), "..")
weir_ui = os.path.join(root, "weir-ui")

# WASM-always (WEIR-A-0030): the demo runs the connectors as wasm guests. Each is a
# guest crate under wasm-fixtures/<kebab>; we build it for wasm32-wasip2 and stage a
# fidius package (weir-<kebab>-pkg) into a connectors/ dir the app resolves via
# WEIR_CONNECTORS_DIR (WEIR-I-0011 S3 / WEIR-I-0010). (kebab, wasm_file)
WASM_CONNECTORS = [
    ("echo", "weir_echo_wasm.wasm"),
    ("slow", "weir_slow_wasm.wasm"),
    ("faulty", "weir_faulty_wasm.wasm"),
    ("arrow-sink", "weir_arrow_sink_wasm.wasm"),
    # F1 ([[WEIR-I-0035]] F1.9): the unified resident connector (mode: poll|tail|ws).
    ("resident", "weir_resident_wasm.wasm"),
]


def _stage_wasm_connectors(connectors_dir):
    """Build each guest for wasm32-wasip2 and stage it as a fidius package."""
    os.makedirs(connectors_dir, exist_ok=True)
    for kebab, wasm_file in WASM_CONNECTORS:
        fixture = os.path.join(root, "wasm-fixtures", kebab)
        if subprocess.run(
            ["cargo", "build", "--target", "wasm32-wasip2", "--release"], cwd=fixture
        ).returncode != 0:
            raise SystemExit(f"wasm build failed for {kebab}")
        art = os.path.join(fixture, "target", "wasm32-wasip2", "release", wasm_file)
        pkg = os.path.join(connectors_dir, f"weir-{kebab}-pkg")
        os.makedirs(pkg, exist_ok=True)
        shutil.copy(art, os.path.join(pkg, wasm_file))
        with open(os.path.join(pkg, "package.toml"), "w") as f:
            f.write(
                f'[package]\nname = "weir-{kebab}-pkg"\nversion = "0.1.0"\n'
                'interface = "connector"\ninterface_version = 1\nruntime = "wasm"\n\n'
                '[metadata]\ncategory = "demo"\n\n'
                f'[wasm]\ncomponent = "{wasm_file}"\ncapabilities = []\n'
            )
    print(f"  ▸ staged {len(WASM_CONNECTORS)} wasm connectors → {connectors_dir}")

ui = angreal.command_group(name="ui", about="build + demo the embedded web UI")


def _trunk_build():
    return subprocess.run(["trunk", "build", "--release"], cwd=weir_ui).returncode


# Demo connections — all self-contained, one per UI state we want to exercise.
# `every` (seconds, fractional) drives them via the real scheduler (WEIR-T-0051).
DEMO_CONNS = [
    # name, source, stream, config, every_secs  (dest is always ArrowSink)
    ("echo-quick", "Echo", "echo", {}, 1.0),
    ("slow-stream", "Slow", "slow", {"sleep_ms": 2500, "rows": 12}, 8.0),
    ("dead-letters", "Faulty", "faulty", {"dead_letter": True}, 3.0),
    ("failing-api", "Faulty", "faulty", {"fail": "fatal", "token": "demo-fail"}, 5.0),
]


@ui()
@angreal.command(name="build", about="trunk build the embedded Leptos UI (release)")
def build():
    raise SystemExit(_trunk_build())


@ui()
@angreal.command(
    name="demo",
    about="build UI + seed demo connections, then start the (scheduler-driven) weir server and leave it running",
)
@angreal.argument(name="port", long="port", takes_value=True, help="HTTP port (default 8787)")
def demo(port="8787"):
    if _trunk_build() != 0:
        raise SystemExit("UI build failed")
    if subprocess.run(["cargo", "build", "-p", "weir-cli"], cwd=root).returncode != 0:
        raise SystemExit("weir build failed")

    weir = os.path.join(root, "target", "debug", "weir")
    db = os.path.join(tempfile.gettempdir(), "weir-demo.db")

    # Stop any prior demo server on this port, then start fresh.
    subprocess.run(["pkill", "-f", f"api --port {port}"], capture_output=True)
    time.sleep(0.5)
    if os.path.exists(db):
        os.remove(db)

    # WASM-always: build + stage the guests; the app resolves them via WEIR_CONNECTORS_DIR.
    connectors_dir = os.path.join(tempfile.gettempdir(), "weir-demo-connectors")
    if os.path.exists(connectors_dir):
        shutil.rmtree(connectors_dir)
    _stage_wasm_connectors(connectors_dir)
    env = {**os.environ, "WEIR_CONNECTORS_DIR": connectors_dir}

    def weir_cmd(*args):
        return subprocess.run([weir, "--db", db, *args], cwd=root, env=env).returncode

    weir_cmd("init")
    for name, source, stream, config, every in DEMO_CONNS:
        weir_cmd(
            "connection", "add", "--name", name, "--source", source,
            "--dest", "ArrowSink", "--stream", stream, "--config", json.dumps(config),
            "--every", str(every),
        )

    # F1 ([[WEIR-I-0035]]) resident demo: a long-lived polling source that emits ~1 row / 2s
    # forever — the always-on "resident • live" tile (and a passive long soak). `--every` is
    # its emit cadence; `weir start` enqueues it once so the server's worker runs it under
    # `run_resident` on boot.
    weir_cmd(
        "connection", "add", "--name", "resident-demo", "--source", "resident",
        "--dest", "ArrowSink", "--stream", "resident", "--config",
        '{"mode":"poll","rows_per_poll":3}',
        "--every", "2.0", "--execution-mode", "resident",
    )
    weir_cmd("start", "--name", "resident-demo")

    # Start the server DETACHED — it owns the scheduler + worker, so it drives the
    # feed on its own. We seed the catalog once it's up, then exit and leave it running.
    base = f"http://localhost:{port}"
    api = subprocess.Popen(
        [weir, "--db", db, "api", "--port", port], cwd=root, env=env, start_new_session=True
    )

    # Wait until the API is accepting requests, then seed the catalog.
    for _ in range(60):
        try:
            urllib.request.urlopen(f"{base}/catalog", timeout=1)
            break
        except Exception:
            if api.poll() is not None:
                raise SystemExit("weir api exited before becoming ready")
            time.sleep(0.5)

    # Catalog (WEIR-I-0010): register the connectors the demo connections use so the
    # source/dest dropdowns are populated. Leave `echo` staged-but-UNregistered as the
    # "Add connectors" demo — pick it from the discover dropdown → Add → it registers.
    for pkg in ("weir-arrow-sink-pkg", "weir-slow-pkg", "weir-faulty-pkg", "weir-resident-pkg"):
        try:
            urllib.request.urlopen(
                urllib.request.Request(
                    f"{base}/catalog/import",
                    data=json.dumps({"package": pkg}).encode(),
                    headers={"content-type": "application/json"},
                    method="POST",
                ),
                timeout=15,
            )
        except Exception as e:
            print(f"  (catalog seed {pkg} failed: {e})")

    print("  ▸ seeded catalog · `echo` left for the 'Add connectors' flow")
    print(f"\n  ▸ weir running (scheduler-driven)  →  {base}   [pid {api.pid}]")
    print(f"    stop it with:  kill {api.pid}    (or: pkill -f 'api --port {port}')\n")
    raise SystemExit(0)
