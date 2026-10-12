"""
Docs tasks: `angreal docs build`, `angreal docs serve` and `angreal docs api`.

The docs are an Astro Starlight site in docs/, with the Colliery theme
starlight-aurora. They need Node 22 or later. The API reference is written by
plissken (plissken.toml) into docs/src/content/docs/reference/api/; it is not
in git.
"""

import shutil
import subprocess
import sys
from pathlib import Path

import angreal  # type: ignore

PROJECT_ROOT = Path(angreal.get_root()).parent
DOCS_ROOT = PROJECT_ROOT / "docs"

docs = angreal.command_group(name="docs", about="documentation site commands")


def _plissken():
    plissken = shutil.which("plissken")
    if not plissken:
        print("plissken not found: the site builds without the API reference", file=sys.stderr)
        return 0
    return subprocess.run([plissken, "render"], cwd=str(PROJECT_ROOT)).returncode


def _npm(*args):
    npm = shutil.which("npm")
    if not npm:
        print("npm not found: install Node 22 or later", file=sys.stderr)
        return 1
    if not (DOCS_ROOT / "node_modules").exists():
        result = subprocess.run([npm, "install", "--no-audit", "--no-fund"], cwd=str(DOCS_ROOT))
        if result.returncode != 0:
            return result.returncode
    return subprocess.run([npm, *args], cwd=str(DOCS_ROOT)).returncode


@docs()
@angreal.command(
    name="build",
    about="render the API reference and build the docs site into docs/dist",
    tool=angreal.ToolDescription(
        "Renders the API reference with plissken (if it is installed), then builds "
        "the Starlight docs site in docs/ into docs/dist. Installs the npm "
        "dependencies on the first run. Fails when a page does not build.",
        risk_level="safe",
    ),
)
def docs_build():
    return _plissken() or _npm("run", "build")


@docs()
@angreal.command(
    name="serve",
    about="serve the docs site with live reload",
    tool=angreal.ToolDescription(
        "Renders the API reference with plissken (if it is installed), then serves "
        "the docs site at http://localhost:4321 and reloads on each change. In a "
        "terminal it runs until Ctrl+C. From a script or an agent, Astro runs it in "
        "the background: stop it with `npx astro dev stop` in docs/.",
        risk_level="safe",
    ),
)
def docs_serve():
    return _plissken() or _npm("run", "dev")


@docs()
@angreal.command(
    name="api",
    about="render the API reference only (plissken render)",
    tool=angreal.ToolDescription(
        "Runs `plissken render`: writes the API reference pages and _sidebar.mjs "
        "into docs/src/content/docs/reference/api/.",
        risk_level="safe",
    ),
)
def docs_api():
    if not shutil.which("plissken"):
        print("plissken not found", file=sys.stderr)
        return 1
    return _plissken()
