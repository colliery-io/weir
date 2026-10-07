# Secrets posture

This page tells you what weir does to keep connector secrets (passwords, API keys, private keys) safe in the
alpha, and what it does not do. Read it before you put a production credential in a connection.

## Summary

Weir hides secrets from the people and the processes that **use** the control plane: API reads, the web UI,
logs and errors, and the WASM connector guests. Weir does **not** encrypt secrets at rest. A literal secret
value is stored in plaintext in the database. To keep a secret out of weir completely, give it as a
[reference](../guides/secret-references.md) (`env:NAME` or `file:/path`).

## Which fields are secret

A field of `source_config` or `dest_config` is secret when one of these is true:

- The connector `config_schema` marks it `airbyte_secret: true` or `format: "password"`. The first-party
  connectors mark their credential fields. A test fails when a credential field of a first-party connector is
  not marked.
- It holds a host-side auth credential: `api_key`, or the field that `basic_password_key`,
  `oauth_client_secret_key`, `oauth_refresh_token_key`, `google_sa_key_key`, `snowflake_private_key_key` or
  `aws_secret_access_key_key` names. See [Connection config](../reference/connection-config.md).

Weir cannot know that a field is secret if the schema does not mark it. For example, a declarative (manifest)
connector has an empty `config_schema`. Put the credentials of such a connector in the `auth_*` fields, which
are always secret.

## Threat model

| Surface | Protected? | What weir does |
|---|---|---|
| API reads, at every role (Read, Write, admin) | Yes | A read gives `__weir_secret_unchanged__` in place of a literal secret. A reference reads back as its text, for example `env:PG_PASSWORD`. |
| Web UI | Yes | The UI shows a stored secret as "unchanged". An edit sends the secret back only if you type a new value or clear it. |
| Logs and run errors | Yes | A failed reference names the field and the reference, not the value. |
| Credential cache keys | Yes | The connector-handle cache and the minted-token caches use salted SHA-256 digests, not the config text. A changed or rotated credential mints a new token. |
| Connector guests (WASM) | Yes | The host removes the secret fields before the config goes into the guest. The host attaches the credential to the egress request. A connection with an unknown `auth_scheme` is refused at create and at run, so its secrets do not go into the guest. |
| Database at rest | **No** | A literal secret is stored in plaintext in the `connections` table. |
| Database backups and copies | **No** | A backup holds the same plaintext. |
| A person with access to the host | **No** | The control-plane host can read the database and run `weir` CLI commands against it. The runner host can read the environment variables and the files that references point to. |
| A Write key | **Partly** | A Write key cannot read a secret. It can **replace** or **clear** a secret, and it can point a connection at a different host that receives the credential. |
| The CLI `weir connection add` | **No** | The CLI writes the full connection to the database. It does not keep stored secrets: a field that you do not give is removed. |

## What you must do

- Protect the database file and its backups as you protect the secrets in them. Limit who can read them.
- Give Write keys only to people that you trust with the credentials of the tenant. Refer to
  [Tenancy posture in the alpha](tenancy-posture.md).
- When you change a connection with the CLI, give all of its secret fields again. To change one field and
  keep the secrets, use `POST /connections` or the web UI.

## When to use a reference

Use a reference (`env:NAME` or `file:/path`) when one of these is true:

- The database or its backups are in a location that you do not control as tightly as the secret.
- A different system already manages the secret (for example a Kubernetes Secret, Vault Agent or a cloud
  secret manager that writes a file or sets a variable).
- You rotate the secret. The runner reads the reference again on each run, so the next run uses the new value.
  You do not change the connection.

Use a literal value only for development, or when the database has the same protection as the secret.

## Design decisions

- **No secret store in weir.** Weir does not store, encrypt or manage secrets. It reads them again on each run
  from the host (WEIR-A-0037).
- **A control plane without plaintext is the goal.** References make this possible today. Encryption at rest of
  the `connections` table is not in the alpha (WEIR-A-0013).
- **Credentials are injected on the host.** The guest never gets the credential (WEIR-A-0033).

## Related pages

- [Give a secret as a reference](../guides/secret-references.md)
- [Connection config](../reference/connection-config.md)
- [Secure the control plane](../guides/secure-control-plane.md)
- [Tenancy posture in the alpha](tenancy-posture.md)
