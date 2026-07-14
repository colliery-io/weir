"""weir distribution image.

`angreal docker build` — multi-stage build of the thin, self-contained weir image:
the host binary (Leptos UI embedded), pre-staged wasm connectors, and the vendored
declarative manifests. `angreal docker run` — run it on :8080.
"""

import os
import subprocess

import angreal

root = os.path.join(angreal.get_root(), "..")

docker = angreal.command_group(name="docker", about="build / run the weir distribution image")


@docker()
@angreal.command(
    name="build",
    about="build the thin weir image (pre-staged connectors + vendored manifests)",
)
@angreal.argument(name="tag", long="tag", takes_value=True, help="image tag (default: weir:dev)")
def build(tag="weir:dev"):
    return subprocess.run(["docker", "build", "-t", tag, "."], cwd=root).returncode


@docker()
@angreal.command(name="run", about="run the weir image, control plane on :8080")
@angreal.argument(name="tag", long="tag", takes_value=True, help="image tag (default: weir:dev)")
@angreal.argument(name="port", long="port", takes_value=True, help="host port (default: 8080)")
def run(tag="weir:dev", port="8080"):
    return subprocess.run(
        ["docker", "run", "--rm", "-p", f"{port}:8080", tag], cwd=root
    ).returncode


# The unified compose.yml is auto-discovered; the `demo` profile adds the weir
# service on top of Postgres (a bare `up`, used by `angreal integration`, is
# Postgres-only).
_DEMO = ["docker", "compose", "--profile", "demo"]


@docker()
@angreal.command(
    name="up",
    about="build + run the demo stack (weir + Postgres SoR) → http://localhost:8080",
)
def up():
    # Builds the image (build: in the compose) and runs the control plane against
    # Postgres. Attached — Ctrl-C to stop, then `angreal docker down`.
    return subprocess.run(_DEMO + ["up", "--build"], cwd=root).returncode


@docker()
@angreal.command(name="down", about="stop the demo stack and remove its volumes")
def down():
    return subprocess.run(_DEMO + ["down", "-v"], cwd=root).returncode
