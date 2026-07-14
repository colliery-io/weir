import angreal
import subprocess
import os
import tempfile

cwd = os.path.join(angreal.get_root(), '..')
test = angreal.command_group(name="test", about="commands for running tests")


def get_crates():
    """Discover crates in the workspace."""
    crates_dir = os.path.join(cwd, 'crates')
    if not os.path.isdir(crates_dir):
        return []
    return [d for d in os.listdir(crates_dir)
            if os.path.isfile(os.path.join(crates_dir, d, 'Cargo.toml'))]


# WASM-always (WEIR-A-0030): connectors run as wasm guests. Build them ONCE here so
# the test suite loads cached artifacts (compile-once-use-many) — cargo's incremental
# cache makes repeat runs no-ops; the testkit skips a rebuild when the artifact is fresh.
# The testkit's default `load()` resolves these from `wasm-fixtures/<name>`. Real connectors
# (rest/rest-dest/postgres/s3) live under `crates/connectors/` and are staged per-test, so they
# aren't listed here.
WASM_FIXTURES = ["echo", "slow", "faulty", "arrow-sink"]


def build_wasm_connectors():
    """Compile every wasm guest connector for wasm32-wasip2 (release), once."""
    for fx in WASM_FIXTURES:
        d = os.path.join(cwd, "wasm-fixtures", fx)
        r = subprocess.run(
            ["cargo", "build", "--target", "wasm32-wasip2", "--release"], cwd=d
        )
        if r.returncode != 0:
            raise SystemExit(f"wasm guest build failed for {fx}")


def _run(cmd):
    """Run a command and exit with its return code."""
    result = subprocess.run(cmd, cwd=cwd)
    raise SystemExit(result.returncode)


def _add_crate_filter(cmd, crate_name, filter_str):
    """Add crate and filter arguments to a cargo command."""
    if crate_name:
        cmd.extend(["-p", crate_name])
    cmd.extend(["--", "--test-threads=1"])
    if filter_str:
        cmd.append(filter_str)
    return cmd


@test()
@angreal.command(name="unit", about="run unit tests (cargo test --lib)")
@angreal.argument(name="crate_name", required=False, help="specific crate to test (default: all)")
@angreal.argument(name="filter", long="filter", short="f", required=False, help="filter for specific tests")
def unit_tests(crate_name="", filter=""):
    cmd = ["cargo", "test", "--lib", "-v"]
    _run(_add_crate_filter(cmd, crate_name, filter))


@test()
@angreal.command(name="integration", about="run integration tests (tests/integration.rs)")
@angreal.argument(name="crate_name", required=False, help="specific crate to test (default: all)")
@angreal.argument(name="filter", long="filter", short="f", required=False, help="filter for specific tests")
def integration_tests(crate_name="", filter=""):
    cmd = ["cargo", "test", "--test", "integration"]
    _run(_add_crate_filter(cmd, crate_name, filter))


@test()
@angreal.command(name="functional", about="run functional tests (tests/functional.rs)")
@angreal.argument(name="crate_name", required=False, help="specific crate to test (default: all)")
@angreal.argument(name="filter", long="filter", short="f", required=False, help="filter for specific tests")
@angreal.argument(
    name="ignored",
    long="ignored",
    takes_value=False,
    help="include #[ignore] tests (e.g. tests needing external services)",
)
def functional_tests(crate_name="", filter="", ignored=False):
    cmd = ["cargo", "test", "--test", "functional"]
    if crate_name:
        cmd.extend(["-p", crate_name])
    cmd.extend(["--", "--test-threads=1"])
    if ignored:
        cmd.append("--ignored")
    if filter:
        cmd.append(filter)
    _run(cmd)


@test()
@angreal.command(name="all", about="run all tests (unit, integration, and functional)")
@angreal.argument(name="crate_name", required=False, help="specific crate to test (default: all)")
@angreal.argument(
    name="ignored",
    long="ignored",
    takes_value=False,
    help="include #[ignore] tests",
)
def all_tests(crate_name="", ignored=False):
    build_wasm_connectors()  # compile-once: tests load cached wasm guests
    cmd = ["cargo", "test", "-v"]
    if crate_name:
        cmd.extend(["-p", crate_name])
    cmd.extend(["--", "--test-threads=1"])
    if ignored:
        cmd.append("--ignored")
    _run(cmd)


@test()
@angreal.command(
    name="connectors",
    about="build the wasm guest connectors (compile-once cache for the test suite)",
)
def build_connectors():
    build_wasm_connectors()
    print("built wasm guest connectors:", ", ".join(WASM_FIXTURES))
    raise SystemExit(0)


@test()
@angreal.command(name="coverage", about="generate code coverage report")
@angreal.argument(
    name="type",
    long="type",
    short="t",
    required=False,
    help="test type to measure: unit, integration, functional, all (default: all)",
)
@angreal.argument(
    name="open",
    long="open",
    short="o",
    takes_value=False,
    help="open HTML report in browser",
)
def coverage(type="all", open=False):
    import webbrowser

    output_dir = os.path.join(cwd, "coverage")

    # Build the test selection flags for cargo-llvm-cov
    test_flags = []
    if type == "unit":
        test_flags = ["--lib"]
    elif type == "integration":
        test_flags = ["--test", "integration"]
    elif type == "functional":
        test_flags = ["--test", "functional"]
    # "all" uses no flags (runs everything)

    # Generate coverage
    run_cmd = ["cargo", "llvm-cov", "--workspace", "--no-report"] + test_flags + ["--", "--test-threads=1"]
    report_html = ["cargo", "llvm-cov", "report", "--workspace", "--html", "--output-dir", output_dir]
    report_lcov = ["cargo", "llvm-cov", "report", "--workspace", "--lcov", "--output-path", os.path.join(output_dir, "lcov.info")]
    report_text = ["cargo", "llvm-cov", "report", "--workspace"]

    for cmd in [run_cmd, report_html, report_lcov, report_text]:
        result = subprocess.run(cmd, cwd=cwd)
        if result.returncode != 0:
            raise SystemExit(result.returncode)

    if open:
        report = os.path.join(output_dir, "html", "index.html")
        webbrowser.open(f"file://{os.path.realpath(report)}")


@test()
@angreal.command(
    name="manifests",
    about="validate the vendored manifest corpus onboards (--live also runs the network test)",
)
@angreal.argument(
    name="live",
    long="live",
    takes_value=False,
    is_flag=True,
    help="also run the live network test (jsonplaceholder -> rest -> arrow-sink)",
)
def manifest_tests(live=False):
    cmd = ["cargo", "test", "-p", "weir-app", "--test", "manifest_corpus", "--", "--nocapture"]
    if live:
        cmd.append("--include-ignored")
    _run(cmd)


@test()
@angreal.command(
    name="connectors-live",
    about="run the live connector integration suite (no-auth + sops-decrypted authed) (WEIR-I-0014)",
)
def connectors_live():
    """Run the full live manifest_corpus suite against real APIs.

    Runs onboarding + the no-auth live connectors + the keyed (authed) connectors.
    Each `secrets/<slug>.enc.json` is sops-decrypted into a private temp dir as
    `<slug>.json`; WEIR_SECRETS_DIR points the keyed test at it, so the repo's
    gitignored plaintext is never touched and no plaintext is written under secrets/.
    Requires SOPS_AGE_KEY_FILE (local) or SOPS_AGE_KEY (CI) for the authed connectors;
    with no bundles / no key, the authed connectors simply skip and the no-auth suite
    still runs. Assertion is functional (rows > 0 over the live API), not data-exact.
    """
    import glob
    import shutil
    import tempfile

    # The keyed test stages the wasm `rest` runtime itself (stage_live_runtime), so we
    # don't pre-build connectors here.
    enc = sorted(glob.glob(os.path.join(cwd, "secrets", "*.enc.json")))
    env = dict(os.environ)
    tmp = None
    if enc:
        tmp = tempfile.mkdtemp(prefix="weir-secrets-")  # 0700 — not world-readable
        for f in enc:
            slug = os.path.basename(f)[: -len(".enc.json")]
            r = subprocess.run(
                ["sops", "-d", os.path.relpath(f, cwd)], cwd=cwd, capture_output=True
            )
            if r.returncode != 0:
                shutil.rmtree(tmp, ignore_errors=True)
                raise SystemExit(
                    f"sops decrypt failed for {os.path.relpath(f, cwd)}:\n{r.stderr.decode()}"
                )
            with open(os.path.join(tmp, f"{slug}.json"), "wb") as out:
                out.write(r.stdout)
        env["WEIR_SECRETS_DIR"] = tmp
        print(f"  decrypted {len(enc)} secret bundle(s) -> {tmp}")
    else:
        print("  no secrets/*.enc.json — running keyed suite (authed connectors will skip)")

    # The whole suite: onboarding + no-auth live + keyed (authed) live. --include-ignored
    # pulls in the network-gated tests; single-threaded (mock/live servers bind ports).
    cmd = [
        "cargo", "test", "-p", "weir-app", "--test", "manifest_corpus",
        "--", "--include-ignored", "--test-threads=1", "--nocapture",
    ]
    try:
        result = subprocess.run(cmd, cwd=cwd, env=env)
    finally:
        if tmp:
            shutil.rmtree(tmp, ignore_errors=True)
    raise SystemExit(result.returncode)


def _wait_health(port):
    import time
    import urllib.request
    for _ in range(60):
        try:
            urllib.request.urlopen(f"http://localhost:{port}/health", timeout=1)
            return True
        except Exception:
            time.sleep(0.5)
    return False


def _seed(port, key):
    """Seed an fx-demo connection (authed) for operations.spec."""
    import json
    import urllib.request
    def post(path, body):
        data = json.dumps(body).encode() if body is not None else b""
        req = urllib.request.Request(
            f"http://localhost:{port}{path}", data=data, method="POST",
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
        try:
            urllib.request.urlopen(req, timeout=15)
        except Exception:
            pass
    post("/catalog/import", {"manifest_name": "frankfurter"})
    post("/catalog/import", {"package": "weir-arrow-sink-pkg"})
    post("/connections", {"name": "fx-demo", "source": "frankfurter",
                          "dest": "ArrowSink", "stream": "latest", "config": {}})
    post("/connections/fx-demo/run", None)


@test()
@angreal.command(
    name="e2e",
    about="UI e2e (Playwright) incl the OIDC door via Dex (WEIR-I-0017 / WEIR-T-0088)",
)
def auth_e2e():
    """Build UI + connectors, bring up Dex, start the locked weir server (key + OIDC),
    run the Playwright suite (both auth doors), tear down."""
    port = "8787"
    connectors = os.path.join(tempfile.gettempdir(), "weir-e2e-connectors")
    db = os.path.join(tempfile.gettempdir(), "weir-e2e.db")
    weir = os.path.join(cwd, "target", "debug", "weir")

    if subprocess.run(["trunk", "build", "--release"], cwd=os.path.join(cwd, "weir-ui")).returncode != 0:
        raise SystemExit("ui build failed")
    subprocess.run(["bash", "scripts/stage-connectors.sh", connectors], cwd=cwd)
    if subprocess.run(["cargo", "build", "-p", "weir-cli"], cwd=cwd).returncode != 0:
        raise SystemExit("weir-cli build failed")
    if subprocess.run(["docker", "compose", "up", "-d", "dex", "--wait"], cwd=cwd).returncode != 0:
        raise SystemExit("dex up failed — is docker running and :5557 free?")

    env = {
        **os.environ,
        "WEIR_CONNECTORS_DIR": connectors,
        "WEIR_MANIFESTS_DIR": os.path.join(cwd, "manifests"),
        "WEIR_DEST_MANIFESTS_DIR": os.path.join(cwd, "dest-manifests"),
        "WEIR_OIDC_ISSUER": "http://localhost:5557/dex",
        "WEIR_OIDC_CLIENT_ID": "weir",
        "WEIR_OIDC_CLIENT_SECRET": "weir-secret",
        "WEIR_OIDC_REDIRECT_URI": f"http://localhost:{port}/auth/callback",
    }
    if os.path.exists(db):
        os.remove(db)
    subprocess.run([weir, "--db", db, "init"], cwd=cwd, env=env)
    out = subprocess.run([weir, "--db", db, "auth", "token", "create", "--name", "e2e", "--admin"],
                         cwd=cwd, env=env, capture_output=True, text=True).stdout
    key = next((w for line in out.splitlines() for w in line.split() if w.startswith("weirk_")), "")

    subprocess.run(["pkill", "-f", f"api --port {port}"], capture_output=True)
    server = subprocess.Popen([weir, "--db", db, "api", "--port", port], cwd=cwd, env=env)
    rc = 1
    try:
        if not _wait_health(port):
            raise SystemExit("weir server did not become healthy")
        _seed(port, key)
        rc = subprocess.run(["npx", "playwright", "test"],
                            cwd=os.path.join(cwd, "e2e"), env={**env, "WEIR_E2E_KEY": key}).returncode
    finally:
        server.terminate()
        subprocess.run(["docker", "compose", "down"], cwd=cwd, capture_output=True)
    raise SystemExit(rc)
