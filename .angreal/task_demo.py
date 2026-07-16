"""Demo pipelines ([[WEIR-T-0162]] / [[WEIR-I-0041]]).

`angreal demo pipelines` stands a representative estate up on a clean weir — six
end-to-end pipelines (HubSpot/Stripe/Sheets/GA4/MSSQL → Snowflake, and Snowflake →
HubSpot rETL) — by driving the control-plane HTTP API, exactly like the UI does. The
estate is **data** (`demo/pipelines.toml`); pointing it at a different estate is an edit
to that file, not this task.

Secret bundles (SOPS, WEIR-S-0018) are decrypted at run time and their fields spliced
into per-side connection config per the spec's `source_secrets` / `dest_secrets`. A
pipeline whose bundle is absent is still **provisioned** (the connection is created) but
flagged in the summary as needing that bundle before its live run — so the estate stands
up today and each pipeline goes live as its account lands.

`angreal demo down` stops the server and clears the demo state.
"""

import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

try:
    import tomllib  # py3.11+
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None

import angreal

root = os.path.join(angreal.get_root(), "..")
SPEC = os.path.join(root, "demo", "pipelines.toml")
PORT = os.environ.get("WEIR_DEMO_PORT", "8090")
BASE = f"http://localhost:{PORT}"
DB = os.path.join(tempfile.gettempdir(), "weir-demo-pipelines.db")
CONNECTORS_DIR = os.path.join(tempfile.gettempdir(), "weir-demo-pipelines-connectors")
PIDFILE = os.path.join(tempfile.gettempdir(), "weir-demo-pipelines.pid")

demo = angreal.command_group(name="demo", about="stand up the demo pipeline estate")


def _load_spec():
    if tomllib is None:
        raise SystemExit("demo pipelines needs Python 3.11+ (tomllib)")
    with open(SPEC, "rb") as f:
        return tomllib.load(f)


def _decrypt_bundles():
    """SOPS-decrypt every secrets/<slug>.enc.json → {slug: {field: value}}. Missing
    bundles are simply absent (their pipelines provision but don't go live)."""
    bundles = {}
    for enc in sorted(glob.glob(os.path.join(root, "secrets", "*.enc.json"))):
        slug = os.path.basename(enc)[: -len(".enc.json")]
        r = subprocess.run(
            ["sops", "-d", os.path.relpath(enc, root)],
            cwd=root,
            capture_output=True,
        )
        if r.returncode != 0:
            print(f"  (skip {slug}: sops decrypt failed — {r.stderr.decode().strip()})")
            continue
        try:
            bundles[slug] = json.loads(r.stdout)
        except json.JSONDecodeError:
            print(f"  (skip {slug}: not valid JSON)")
    return bundles


def _resolve(mapping, bundles, out, missing):
    """Splice `config_key = "slug.field"` refs into `out`; record absent bundles/fields."""
    for config_key, ref in (mapping or {}).items():
        slug, _, field = ref.partition(".")
        val = bundles.get(slug, {}).get(field)
        if val is None:
            missing.add(slug)
            continue
        out[config_key] = val


def _api(method, path, token, body=None, timeout=30):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(
        f"{BASE}{path}",
        method=method,
        data=data,
        headers={
            "authorization": f"Bearer {token}",
            "content-type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode()
            return resp.status, (json.loads(raw) if raw.strip() else None)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def _weir(*args, env):
    return subprocess.run(
        [os.path.join(root, "target", "debug", "weir"), "--db", DB, *args],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
    )


def _stop_server():
    subprocess.run(["pkill", "-f", f"api --port {PORT}"], capture_output=True)
    if os.path.exists(PIDFILE):
        os.remove(PIDFILE)


@demo()
@angreal.command(
    name="pipelines",
    about="onboard six representative pipelines onto a clean weir (idempotent)",
    long_about=__doc__,
)
@angreal.argument(
    name="keep_db",
    long="keep-db",
    takes_value=False,
    is_flag=True,
    help="reuse the existing demo db instead of starting clean (idempotent re-provision)",
)
@angreal.argument(
    name="validate",
    long="validate",
    takes_value=False,
    is_flag=True,
    help="after provisioning, run each connection once and report rows landed (the scripted smoke)",
)
def demo_pipelines(keep_db=False, validate=False):
    spec = _load_spec()

    print("▸ decrypting secret bundles (SOPS)")
    bundles = _decrypt_bundles()
    print(f"  bundles present: {', '.join(sorted(bundles)) or '(none)'}")

    # Build weir + stage the compiled guests; manifests are read from manifests/.
    print("▸ building weir + staging connectors")
    if subprocess.run(["cargo", "build", "-p", "weir-cli"], cwd=root).returncode != 0:
        raise SystemExit("weir build failed")
    if os.path.exists(CONNECTORS_DIR):
        shutil.rmtree(CONNECTORS_DIR)
    if subprocess.run(
        ["bash", os.path.join(root, "scripts", "stage-connectors.sh"), CONNECTORS_DIR],
        cwd=root,
    ).returncode != 0:
        raise SystemExit("connector staging failed")

    env = {
        **os.environ,
        "WEIR_CONNECTORS_DIR": CONNECTORS_DIR,
        "WEIR_MANIFESTS_DIR": os.path.join(root, "manifests"),
    }

    _stop_server()
    if not keep_db and os.path.exists(DB):
        os.remove(DB)
    _weir("init", env=env)

    # Mint an admin key for the API (printed once); the whole demo drives the API with it.
    tok = _weir("auth", "token", "create", "--name", "demo", "--admin", env=env)
    token = _extract_token(tok.stdout)
    if not token:
        raise SystemExit(f"could not mint demo API token:\n{tok.stdout}\n{tok.stderr}")

    # Start the API detached; it owns the scheduler + worker.
    api = subprocess.Popen(
        [os.path.join(root, "target", "debug", "weir"), "--db", DB, "api", "--port", PORT],
        cwd=root,
        env=env,
        start_new_session=True,
    )
    with open(PIDFILE, "w") as f:
        f.write(str(api.pid))
    _wait_ready(api, token)

    # ── Onboard connectors (POST /catalog/import) ──────────────────────────────
    print("▸ onboarding connectors")
    for c in spec.get("connector", []):
        kind, name = c["kind"], c["name"]
        if kind == "manifest":
            body = {"manifest_name": name}
        elif kind == "dest_manifest":
            body = {"dest_manifest_name": name}
        elif kind == "package":
            body = {"package": c["package"]}
        else:
            raise SystemExit(f"unknown connector kind `{kind}` for `{name}`")
        status, resp = _api("POST", "/catalog/import", token, body)
        ok = status in (200, 201)
        print(f"  {'✓' if ok else '✗'} {name} ({kind}) [{status}]")
        if not ok:
            print(f"      {resp}")

    # ── Create the six connections (POST /connections) ─────────────────────────
    print("▸ creating connections")
    summary = []
    for conn in spec.get("connection", []):
        missing = set()
        source_config = dict(conn.get("source_extra", {}))
        dest_config = dict(conn.get("dest_extra", {}))
        _resolve(conn.get("source_secrets"), bundles, source_config, missing)
        _resolve(conn.get("source_extra_secrets"), bundles, source_config, missing)
        _resolve(conn.get("dest_secrets"), bundles, dest_config, missing)

        dto = {
            "name": conn["name"],
            "source": conn["source"],
            "dest": conn["dest"],
            "stream": conn["stream"],
            "sync_mode": conn.get("sync_mode", "full_refresh"),
            "write_mode": conn.get("write_mode", "append"),
        }
        if source_config:
            dto["source_config"] = source_config
        if dest_config:
            dto["dest_config"] = dest_config
        if conn.get("business_keys"):
            dto["business_keys"] = conn["business_keys"]
        if conn.get("cursor_field"):
            dto["cursor_field"] = conn["cursor_field"]

        status, resp = _api("POST", "/connections", token, dto)
        ok = status in (200, 201)
        state = "provisioned" if ok else f"FAILED [{status}]"
        if ok and missing:
            state = f"provisioned — live run needs bundle(s): {', '.join(sorted(missing))}"
        summary.append((conn["name"], ok, state))
        print(f"  {'✓' if ok else '✗'} {conn['name']}: {state}")
        if not ok:
            print(f"      {resp}")

    created = sum(1 for _, ok, _ in summary if ok)
    print(f"\n▸ {created}/{len(summary)} connections provisioned  →  {BASE}   [pid {api.pid}]")

    if validate:
        # The scripted smoke ([[WEIR-T-0162]] AC3): run each provisioned pipeline once and
        # report rows landed. For a Snowflake-dest pipeline `rows_written` is the count the
        # warehouse accepted — the row-count read-back, straight off the run. A pipeline
        # whose bundle is absent fails here and is reported (self-arming, like the live suite).
        print("\n▸ validating — running each pipeline once")
        failed = _validate(spec, token)
        _post_provision_hint(created, len(summary))
        raise SystemExit(0 if (created == len(summary) and not failed) else 1)

    print("  run each pipeline once with:  angreal demo pipelines --validate")
    print("  stop with:  angreal demo down")
    # Non-zero if any connection failed to provision (a real error, not a missing bundle).
    raise SystemExit(0 if created == len(summary) else 1)


def _validate(spec, token):
    """Run each connection once, poll to a terminal state, report rows landed. Returns
    the list of pipelines that did not complete cleanly."""
    failed = []
    for conn in spec.get("connection", []):
        name = conn["name"]
        status, _ = _api("POST", f"/connections/{name}/run", token)
        if status not in (200, 201):
            failed.append(name)
            print(f"  ✗ {name}: could not enqueue [{status}]")
            continue
        row = _poll_run(name, token)
        if row is None:
            failed.append(name)
            print(f"  ✗ {name}: run did not settle")
        elif row.get("state") == "done":
            print(f"  ✓ {name}: {row.get('rows_written', 0)} rows landed")
        else:
            failed.append(name)
            print(f"  ✗ {name}: {row.get('state')} — {row.get('error') or ''}")
    return failed


def _poll_run(name, token, tries=120):
    """Poll a connection's run history until the latest run reaches a terminal state."""
    for _ in range(tries):
        status, rows = _api("GET", "/runs", token)
        if status == 200 and isinstance(rows, list):
            mine = [r for r in rows if r.get("connection") == name]
            if mine:
                latest = mine[0]
                if latest.get("state") in ("done", "failed"):
                    return latest
        time.sleep(1)
    return None


def _post_provision_hint(created, total):
    print(f"\n▸ {created}/{total} provisioned  →  {BASE}")
    print("  stop with:  angreal demo down")


def _extract_token(stdout):
    # `weir auth token create` prints the key once; grab the token-looking line.
    for line in stdout.splitlines():
        line = line.strip()
        # Keys are long opaque strings; take the last whitespace token of a line that has one.
        for word in line.split():
            if len(word) >= 24 and all(ch.isalnum() or ch in "-_." for ch in word):
                return word
    return None


def _wait_ready(api, token):
    for _ in range(120):
        if api.poll() is not None:
            raise SystemExit("weir api exited before becoming ready")
        try:
            status, _ = _api("GET", "/catalog", token, timeout=1)
            if status == 200:
                return
        except Exception:
            pass
        time.sleep(0.5)
    raise SystemExit("weir api did not become ready")


@demo()
@angreal.command(name="down", about="stop the demo server and clear its state")
def down():
    _stop_server()
    for path in (DB, CONNECTORS_DIR):
        if os.path.isdir(path):
            shutil.rmtree(path, ignore_errors=True)
        elif os.path.exists(path):
            os.remove(path)
    print("▸ demo stopped, state cleared")
    raise SystemExit(0)
