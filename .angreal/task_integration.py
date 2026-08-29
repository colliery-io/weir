"""Integration-test resources for weir.

Stands up the test estate the `#[ignore]`d integration tests run against, from the
unified `compose.yml` (auto-discovered). The test-only services (MSSQL + seed, Dex,
MinIO + seed) sit behind the `integration` compose profile ([[WEIR-T-0173]]) so the
demo path (`angreal docker up` / `--profile demo`) starts only weir + Postgres;
these commands activate the profile explicitly. (CI instead provisions a
Postgres-only GitHub Actions service container — its integration estate is
narrower than this local one.)
"""

import os
import subprocess

import angreal

cwd = os.path.join(angreal.get_root(), "..")
integration = angreal.command_group(
    name="integration", about="manage integration-test resources (docker-compose)"
)

# The `integration` profile carries the test-only services; Postgres is unprofiled
# (both demo and integration need it), so this starts the full estate.
_COMPOSE = ["docker", "compose", "--profile", "integration"]


def _run(cmd):
    return subprocess.run(cmd, cwd=cwd).returncode


@integration()
@angreal.command(name="up", about="start integration resources (Postgres, MSSQL, MinIO, Dex) and wait until healthy")
def up():
    raise SystemExit(_run([*_COMPOSE, "up", "-d", "--wait"]))


@integration()
@angreal.command(name="down", about="stop integration resources and remove their volumes")
def down():
    raise SystemExit(_run([*_COMPOSE, "down", "-v"]))


@integration()
@angreal.command(name="status", about="show integration resource status")
def status():
    raise SystemExit(_run([*_COMPOSE, "ps"]))


@integration()
@angreal.command(
    name="test",
    about="bring resources up, then run the (ignored) integration tests against them",
)
def test():
    if _run([*_COMPOSE, "up", "-d", "--wait"]) != 0:
        raise SystemExit(1)
    # `--ignored` runs only the integration tests (gated behind real resources);
    # resources are left running afterwards for iteration — `down` to clean up.
    raise SystemExit(_run(["cargo", "test", "--workspace", "--", "--ignored"]))
