"""Portable control-plane schema (diesel-dualdb).

`angreal schema gen` — regenerate the per-backend migrations + `schema.rs` under
`crates/weir-schema/schema/generated/` from the logical DDL in
`crates/weir-schema/schema/migrations/`. The generated output is committed; rerun
this whenever the logical migrations change. ([[WEIR-I-0013]])
"""

import os
import shutil
import subprocess

import angreal

root = os.path.join(angreal.get_root(), "..")

schema = angreal.command_group(name="schema", about="portable control-plane schema (diesel-dualdb)")


@schema()
@angreal.command(
    name="gen",
    about="regenerate per-backend migrations + schema.rs from the logical DDL",
)
def gen():
    mig = os.path.join(root, "crates", "weir-schema", "schema", "migrations")
    out = os.path.join(root, "crates", "weir-schema", "schema", "generated")
    # Prefer an installed `diesel-dualdb-schema`; fall back to the local
    # diesel-dualdb workspace (the crate's CLI subpackage).
    if shutil.which("diesel-dualdb-schema"):
        cmd = ["diesel-dualdb-schema", mig, out]
    else:
        cmd = [
            "cargo", "run", "--quiet",
            "--manifest-path", "../diesel-dualdb/Cargo.toml",
            "-p", "diesel-dualdb-cli", "--", mig, out,
        ]
    return subprocess.run(cmd, cwd=root).returncode
