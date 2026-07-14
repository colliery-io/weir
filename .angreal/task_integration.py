"""Integration-test resources for weir.

Stands up the Postgres that the WEIR-I-0004 conformance tests run against, from
the unified `compose.yml` (auto-discovered). A bare `docker compose up` brings up
Postgres only — the `weir` service is behind the `demo` profile, so this never
builds the image. `angreal docker up` adds it. Local devs use `angreal integration
up` before running the `#[ignore]`d integration tests; CI uses a service container.
"""

import os
import subprocess

import angreal

cwd = os.path.join(angreal.get_root(), "..")
integration = angreal.command_group(
    name="integration", about="manage integration-test resources (docker-compose)"
)


def _run(cmd):
    return subprocess.run(cmd, cwd=cwd).returncode


@integration()
@angreal.command(name="up", about="start integration resources (Postgres) and wait until healthy")
def up():
    raise SystemExit(_run(["docker", "compose", "up", "-d", "--wait"]))


@integration()
@angreal.command(name="down", about="stop integration resources and remove their volumes")
def down():
    raise SystemExit(_run(["docker", "compose", "down", "-v"]))


@integration()
@angreal.command(name="status", about="show integration resource status")
def status():
    raise SystemExit(_run(["docker", "compose", "ps"]))


@integration()
@angreal.command(
    name="test",
    about="bring resources up, then run the (ignored) integration tests against them",
)
def test():
    if _run(["docker", "compose", "up", "-d", "--wait"]) != 0:
        raise SystemExit(1)
    # `--ignored` runs only the integration tests (gated behind real resources);
    # resources are left running afterwards for iteration — `down` to clean up.
    raise SystemExit(_run(["cargo", "test", "--workspace", "--", "--ignored"]))
