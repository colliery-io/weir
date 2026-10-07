# Give a secret as a reference

Use a reference to keep a secret value out of weir. Weir stores the reference. The host that runs the
connector reads the value each time that a run starts.

## Syntax

Write the reference in place of the value of a secret field:

| Reference | The host reads |
|---|---|
| `env:NAME` | The environment variable `NAME`. `NAME` starts with a letter or `_`, and contains only letters, digits and `_`. |
| `file:/path` | The file at `/path`. The path must be absolute. Weir removes one newline at the end of the file. The file must be 1 MiB or smaller. |

A value that does not agree with this syntax is a literal value. For example, `env:` and `file:relative` are
literal values.

## Procedure

1. Make the secret available on each host that runs connectors (the runner). For example, set an environment
   variable, or mount a secret file:

    ```bash
    export PG_PASSWORD='…'
    ```

2. Put the reference in a secret field of the connection:

    ```bash
    curl -X POST "$WEIR/connections" \
      -H "authorization: Bearer $WEIR_KEY" -H 'content-type: application/json' \
      -d '{
        "name": "orders", "source": "Echo", "dest": "postgres", "stream": "echo",
        "dest_config": {"host": "db", "table": "orders", "password": "env:PG_PASSWORD"}
      }'
    ```

3. Start a run. The host reads `PG_PASSWORD` when it builds the credential for the run.

## Rules

- Use a reference in a secret field only. A field is secret when the connector schema marks it
  `airbyte_secret: true` or `format: "password"`, or when it holds an auth credential (for example `api_key`, or
  the field that `basic_password_key` names). See [Connection config](../reference/connection-config.md).
  If you put a reference in a field that is not secret, `POST /connections` gives `400`.
- The API does not read the reference. A read of the connection gives the reference text, for example
  `env:PG_PASSWORD`. A literal secret value is still hidden as `__weir_secret_unchanged__`.
- The host reads the reference again for each run. To rotate a secret, change the variable or the file. The
  next run uses the new value. You do not change the connection.
- If the variable is not set or is empty, or the host cannot read the file, the run fails. The error gives the
  field and the reference. It does not give the value.

## Notes

- The environment and the files are those of the runner process, not of the API process.
- An OAuth2 or Google service-account token that the host minted with the old secret can stay in use until it
  expires.
