"""Manage sops-encrypted connector test secrets (WEIR-I-0014).

Connector API keys for the keyed integration suite live as sops-encrypted
`secrets/<slug>.enc.json` bundles, committed in ciphertext and decryptable by any
developer whose age public key is in `.sops.yaml`. See secrets/README.md for the
full onboarding/rotation flow. The decrypt-wrapped test run is `angreal test
connectors-live`.
"""

import glob
import os
import subprocess

import angreal

root = os.path.join(angreal.get_root(), "..")
secrets = angreal.command_group(
    name="secrets", about="manage sops-encrypted connector test secrets"
)

# Stub written for a brand-new bundle; the dev replaces it via `sops edit`.
_STUB = (
    "{\n"
    '  "_note": "Connection-config overlay for this connector. Keys match the fields\\n'
    ' the connector needs (e.g. api_key; or client_id/client_secret/refresh_token for\\n'
    ' OAuth). Optional __stream picks the stream. Secret-bearing values are encrypted\\n'
    ' per .sops.yaml; metadata like __stream stays plaintext.",\n'
    '  "api_key": "replace-me"\n'
    "}\n"
)


@secrets()
@angreal.command(
    name="edit",
    about="edit (or create) a sops-encrypted connector secret bundle: secrets/<slug>.enc.json",
)
@angreal.argument(
    name="slug",
    required=True,
    help="connector slug — e.g. 'github' for secrets/github.enc.json (matches manifests/<slug>.yaml)",
)
def secrets_edit(slug):
    """Open secrets/<slug>.enc.json in $EDITOR via sops.

    Requires SOPS_AGE_KEY_FILE to point at your age private key (typically
    ~/.config/sops/age/keys.txt). See secrets/README.md for onboarding.
    """
    rel = os.path.join("secrets", f"{slug}.enc.json")
    target = os.path.join(root, rel)
    # sops 3.10+ `edit` only opens already-encrypted files. For a new bundle, write a
    # stub at the target path (which matches .sops.yaml's path_regex so the recipient
    # list resolves), encrypt it in place, then drop into `sops edit`.
    if not os.path.exists(target):
        print(f"  creating new encrypted bundle {rel}")
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "w") as f:
            f.write(_STUB)
        r = subprocess.run(["sops", "--encrypt", "--in-place", rel], cwd=root)
        if r.returncode != 0:
            # Don't leave a plaintext stub lying around if encryption failed.
            os.remove(target)
            raise SystemExit(r.returncode)
        print("  bundle created; opening for edit so you can replace the placeholder.")
    raise SystemExit(subprocess.run(["sops", "edit", rel], cwd=root).returncode)


@secrets()
@angreal.command(
    name="updatekeys",
    about="re-encrypt every secrets/*.enc.json to the current .sops.yaml recipient list",
)
def secrets_updatekeys():
    """Run after adding or removing a recipient in `.sops.yaml`.

    `sops updatekeys` re-encrypts each bundle's data key to the new recipient list
    without touching the underlying secret values.
    """
    files = sorted(glob.glob(os.path.join(root, "secrets", "*.enc.json")))
    if not files:
        print("  no secrets/*.enc.json — nothing to do.")
        raise SystemExit(0)
    for f in files:
        rel = os.path.relpath(f, root)
        print(f"  updatekeys {rel}")
        r = subprocess.run(["sops", "updatekeys", "--yes", rel], cwd=root)
        if r.returncode != 0:
            raise SystemExit(r.returncode)
    raise SystemExit(0)
