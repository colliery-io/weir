# Connector test secrets (sops-encrypted)

API keys for the **keyed live integration suite** (`keyed_manifests_run_live`,
[[WEIR-I-0014]]) live here as **sops-encrypted** `secrets/<slug>.enc.json` bundles.
They are committed to git in ciphertext; each authorized developer decrypts locally
with their own **age** private key. There is **no shared symmetric key** passed
between humans — onboarding is "you generate a keypair locally, you PR your *public*
key, a recipient adds you to the list."

`<slug>` = the manifest file name (e.g. `todoist` for `manifests/todoist.yaml`). Each
bundle is a **per-connection config overlay** — exactly what you'd type into a
connection's config field. The secret-bearing values are encrypted; metadata stays
plaintext so diffs are reviewable.

Plaintext form of a bundle:

```json
{
  "api_key": "ghp_your_real_token",
  "__stream": "issues"
}
```

- The credential is injected **host-side** by the egress policy ([[WEIR-A-0033]]) per the
  manifest's declared auth scheme. For **OAuth2** connectors the keys are `client_id` /
  `client_secret` (+ `refresh_token` for the refresh-token grant). For **session-token**,
  supply whatever the login needs.
- `"__stream"` (optional) picks the stream (default: the manifest's first); `"_note"` is
  ignored. These stay plaintext (they aren't secrets).

## One-time per-developer setup

1. Install the tooling.

   macOS: `brew install sops age`
   Linux: `sudo apt install age` + grab sops from https://github.com/getsops/sops/releases

2. Generate your age keypair:

   ```sh
   mkdir -p ~/.config/sops/age
   age-keygen -o ~/.config/sops/age/keys.txt
   ```

   The file holds both keys. The line starting `# public key: age1...` is what you share.

3. PR your pubkey into `.sops.yaml` at the repo root, under `age:`:

   ```yaml
   age:
     - age1existing0recipient   # existing
     - age1your0new0pubkey0here # you
   ```

   Once merged, a current recipient runs `angreal secrets updatekeys` and commits the
   refreshed ciphertext.

4. Point sops at your private key (add to your shell rc so it persists):

   ```sh
   export SOPS_AGE_KEY_FILE=~/.config/sops/age/keys.txt
   ```

## Running the suite

```sh
angreal test connectors-live
```

This decrypts every `secrets/*.enc.json` into a private temp dir and runs
`keyed_manifests_run_live` against it (`WEIR_SECRETS_DIR` points the test at the temp
dir — no plaintext is ever written under `secrets/`). Connectors **with** a decrypted
bundle run end-to-end (asserted rows > 0); the rest are skipped. With no bundles (or no
age key), the suite is a no-op — so forks and un-onboarded contributors stay green.

## Editing / adding a secret

```sh
angreal secrets edit github      # opens secrets/github.enc.json in $EDITOR via sops
```

For a brand-new connector the task writes a stub, encrypts it in place, then opens it —
replace the placeholder with the real config and save. sops's per-value encryption keeps
the file structure intact, so PR review shows exactly which value changed.

## Rotating a key

1. Rotate at the vendor.
2. `angreal secrets edit <slug>`, paste the new value, save.
3. Commit + push. Other developers pull and decrypt with their existing age key — no
   human-to-human key exchange.

## Onboarding / removing a developer

- **Add:** the new dev does steps 2–3 above. Any current recipient merges and runs
  `angreal secrets updatekeys` (re-encrypts the data key to the new recipient list
  without touching values), then commits.
- **Remove:** delete their line from `.sops.yaml`, run `angreal secrets updatekeys`,
  commit. **Then rotate any secret they could have read** — they may already hold a local
  clone of the old ciphertext, and removal isn't retroactive.

## Bootstrapping the first bundle

No bundle ships pre-encrypted (there's no first recipient baked in). After your pubkey is
in `.sops.yaml`:

```sh
angreal secrets edit <slug>      # writes a stub, encrypts, opens for edit
git add secrets/<slug>.enc.json
git commit -m "secrets: <slug> connector keys"
```

The path matters: sops resolves the `.sops.yaml` creation rule against the path you give
it, so the file must live at `secrets/<slug>.enc.json` (matching the `path_regex`) for the
age recipients to resolve.

## What's tracked vs ignored

- `secrets/*.enc.json` (ciphertext) + `*.example.json` templates + this README → **tracked**.
- `secrets/*.json` (plaintext) → **gitignored** — never commit a decrypted bundle.

## Coverage notes

Auth is injected **host-side** ([[WEIR-A-0033]]) for every scheme the runtime covers:
**bearer**, **header api-key**, **query-param api-key**, **OAuth2** (refresh-token +
client-credentials), and **session-token**. Pagination (page / offset / cursor), templated
`url_base`, datetime-incremental, and **partition routers** (list / substream) are all
supported. See [[WEIR-S-0016]] for the live construct-by-construct coverage ledger; a
connector using a construct still marked ❌ there won't pass yet even with a valid key.
