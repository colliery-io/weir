# The connector model

A data platform lives or dies by its connectors, and connectors are, by nature, **other people's code** talking
to **other people's systems**. weir's central bet is to run that code as **WASM components** — so it can be
untrusted, portable, and safe by construction.

## Sandboxed by default

Every connector is a `wasm32-wasip2` component. Inside the sandbox there is **no filesystem**, no ambient
network, no host process access. The only reach to the outside world is **network egress**, and only the kind the
host grants: a connector declares a capability (`http` or `tcp`), and the runtime authorises exactly that. A
connector that tries anything else simply can't — the capability isn't there to use.

This is what makes running a community connector corpus tenable: a misbehaving or malicious connector is boxed in
by the runtime, not by trust.

## Secrets never enter the guest

Connectors talk to authenticated APIs, but they never see the credential. When a connector makes an outbound
request, the **host** injects the secret into that request on the way out — the signing/auth happens outside the
sandbox. The guest sends a plain request; the host decorates it.

And weir doesn't *manage* those secrets either: **secrets are off-platform**. weir consumes rotated credentials
provisioned elsewhere; it is not a secrets store. That keeps the blast radius small and the platform out of the
key-custody business.

## Two kinds of connector, one runtime

- **Declarative (low-code) connectors** are small YAML **manifests** describing a REST source (or destination).
  They run on a **shared** WASM runtime — no compilation, no per-connector binary. The vendored corpus in
  `manifests/` is all of this shape. Onboarding one bakes it onto the runtime.
- **Full-code connectors** are Rust crates compiled to `wasm32-wasip2` — for anything a manifest can't express
  (a database's replication protocol, a bespoke API). They implement the same [contract](../reference/connector-contract.md).

Both present identically to the engine: same trait, same record batches, same sandbox. The difference is only in
how much you had to write, and where it's built.

## Why WASM (and not native plugins or subprocesses)

Native plugins share the host's address space and trust; subprocesses need their own sandboxing and IPC. A WASM
component gives a **hard, portable boundary** with a typed interface (WIT) for free, runs the same on any host,
and can be distributed as a single artifact. The cost — a marshalling boundary and no arbitrary syscalls — is
exactly the constraint that makes third-party connectors safe to run.
