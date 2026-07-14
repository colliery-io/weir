import angreal
import os
import subprocess
import tempfile

cwd = os.path.join(angreal.get_root(), '..')


def _wait_health(port):
    """Poll /health until the server answers (or give up)."""
    import time
    import urllib.request
    for _ in range(60):
        try:
            urllib.request.urlopen(f"http://localhost:{port}/health", timeout=1)
            return True
        except Exception:
            time.sleep(0.5)
    return False


@angreal.command(
    name="soak",
    about="stand up weir + a connector fleet and soak it, asserting health invariants (WEIR-I-0023)",
    long_about="""Builds weir-cli + weir-soak, starts a local weir server, provisions a fleet of
connectors (local echo/slow, plus postgres + best-effort live REST for the full run), soaks for a
bounded duration asserting throughput / bounded-queue / bounded-dead-letters / liveness, and tears
down. Exits non-zero on any invariant breach. `--smoke` runs a fast, fully-local, docker-free soak
for CI. The same weir-soak binary points at any running weir via --base-url (compose / kind / k8s),
so this task is just the compose-shaped entry point.""",
)
@angreal.argument(name="mode", long="mode", required=False,
                  help="'smoke' = fast, fully-local, no-docker (CI); 'full' = + postgres + live REST (default)")
@angreal.argument(name="duration", long="duration", required=False,
                  help="soak seconds (default 90; smoke 15)")
@angreal.argument(name="fleet", long="fleet", required=False,
                  help="local echo/slow connections (default 8; smoke 4)")
@angreal.argument(name="resident", long="resident", required=False,
                  help="resident (long-lived, F1) connections to start + soak (default 4 full; 0 smoke)")
def soak(mode=None, duration=None, fleet=None, resident=None):
    smoke = (mode == "smoke")
    port = "8799"
    db = os.path.join(tempfile.gettempdir(), "weir-soak.db")
    connectors = os.path.join(tempfile.gettempdir(), "weir-soak-connectors")
    weir = os.path.join(cwd, "target", "debug", "weir")
    soak_bin = os.path.join(cwd, "target", "debug", "weir-soak")
    pg_port = os.environ.get("WEIR_PG_HOST_PORT", "5432")

    # Build the server + the soak driver, and stage the wasm connectors the server loads.
    # (stage-connectors failing silently → an empty dir → every run fails "plugin not found".)
    if subprocess.run(["bash", "scripts/stage-connectors.sh", connectors], cwd=cwd).returncode != 0:
        raise SystemExit("stage-connectors failed — the server would have no connectors to load")
    if subprocess.run(["cargo", "build", "-p", "weir-cli", "-p", "weir-soak"], cwd=cwd).returncode != 0:
        raise SystemExit("build failed")

    # Real Postgres for the full soak (skipped for --smoke → fully local + deterministic).
    pg_up = False
    if not smoke:
        pg_up = subprocess.run(
            ["docker", "compose", "up", "-d", "postgres", "--wait"], cwd=cwd
        ).returncode == 0
        if not pg_up:
            print("  postgres unavailable — soaking without the pg pair")

    env = {
        **os.environ,
        "WEIR_CONNECTORS_DIR": connectors,
        "WEIR_MANIFESTS_DIR": os.path.join(cwd, "manifests"),
        "WEIR_DEST_MANIFESTS_DIR": os.path.join(cwd, "dest-manifests"),
    }
    if os.path.exists(db):
        os.remove(db)
    subprocess.run([weir, "--db", db, "init"], cwd=cwd, env=env)
    out = subprocess.run(
        [weir, "--db", db, "auth", "token", "create", "--name", "soak", "--admin"],
        cwd=cwd, env=env, capture_output=True, text=True,
    ).stdout
    key = next((w for line in out.splitlines() for w in line.split() if w.startswith("weirk_")), "")

    subprocess.run(["pkill", "-f", f"api --port {port}"], capture_output=True)
    server = subprocess.Popen([weir, "--db", db, "api", "--port", port], cwd=cwd, env=env)
    rc = 1
    try:
        if not _wait_health(port):
            raise SystemExit("weir server did not become healthy")
        args = [
            soak_bin,
            "--base-url", f"http://localhost:{port}",
            "--admin-key", key,
            "--duration", duration or ("20" if smoke else "90"),
            "--fleet", fleet or ("4" if smoke else "8"),
            "--resident", resident or ("0" if smoke else "4"),
        ]
        if smoke:
            # Fine-grained windows so a short CI run still has a real throughput signal.
            args += ["--window", "2", "--no-postgres", "--no-live-rest"]
        elif pg_up:
            args += ["--pg-url", f"postgres://weir:weir@localhost:{pg_port}/weir"]
        else:
            args += ["--no-postgres"]
        rc = subprocess.run(args, cwd=cwd, env=env).returncode
    finally:
        server.terminate()
        if pg_up:
            subprocess.run(["docker", "compose", "down"], cwd=cwd, capture_output=True)
    raise SystemExit(rc)
