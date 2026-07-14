# Delivery guarantees

Data movement has to survive crashes, retries, and duplicate work without losing or corrupting records. weir's
answer is **checkpointed at-least-once delivery** with **idempotent writes** — which together give
effectively-once results. Here's why that holds.

## Checkpoints, transactionally

The engine reads a source in batches and writes them to the destination, and periodically **checkpoints**. A
checkpoint is one database transaction that atomically: advances the stream's **cursor**, records an **outbox**
entry, and flushes the **dead-letters** accumulated since the last one. Because it's one transaction, there is no
state where the cursor moved but the outbox didn't, or vice versa.

If the process dies mid-run, nothing is half-committed. The next time the work unit is claimed, the engine
resumes from the last committed cursor — the batches since then are simply re-read.

## At-least-once, made effectively-once

Resuming by re-reading means a batch can be delivered **more than once** (a crash between writing and
checkpointing replays it). weir doesn't try to make delivery exactly-once — that's a distributed-systems tar pit.
Instead it makes the **writes idempotent**: with an `Upsert` write mode keyed on business keys, re-writing the
same record is a no-op-shaped update, not a duplicate. At-least-once delivery + idempotent application =
effectively-once results, without a distributed commit.

This is also why the CDC path applies changes **in order** and carries each row's key with it: replaying an
`insert → update → delete` sequence lands in the same final state every time.

## Leases make concurrency safe

Work units are claimed under a **lease**. Many workers — across many nodes — can pull from the same queue; a
lease ensures only one executes a given unit at a time, and an expired lease (a dead worker) returns the unit to
the queue for another. The same lease mechanism elects a single **scheduler leader**, so replicas never
double-enqueue. Concurrency is safe because the queue, not coordination, arbitrates it.

## Dead-letters: bad records don't stop the run

A record that can't be processed — an uncoercible value, a malformed change, a rejected write — is **dead-lettered**
with a reason, and the run continues. One poison record never blocks a stream. Dead-letters are visible per
connection (`/connections/{name}/dead-letters`), so an operator can see and fix the shape without the sync
grinding to a halt.
