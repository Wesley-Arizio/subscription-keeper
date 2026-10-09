# Solana Recurring Payments Keeper: DESIGN

Oct 5, 2026 · @Wesley

## Problem

Solana's official Subscriptions & Allowances program authorizes recurring pulls but never triggers them, and Solana has no native scheduler. Someone must submit every charge, every cycle.

The Solana Foundation shipped the program on 2 June 2026 (program ID `De1egAFMkMWZSN5rYXRj9CAdheBamobVNubTsi9avR44`), built by Moonsong Labs, audited by Cantina and Spearbit. The repo `solana-program/subscriptions` ships the on-chain program, Codama-generated Rust and TypeScript clients, and a demo web app whose API exposes a manual `/plans/collect` endpoint. A button, not a keeper.

The repo's own security notes name the harder half: on-chain "Active" is not proof of collectability. A subscriber can revoke the SPL approval, or freeze, empty or close their token account, and pulls then fail while the subscription still reads active. Every integrator therefore rebuilds the same operational layer: schedule the pulls, detect failures, classify them, retry, apply grace, notify.

No shared implementation of that layer exists. The six launch design partners (Helius, Confirmo, Dynamic, Majority, Mesh, Meow) each build collection into their own closed stacks. The public attempts found (Subly, Recur, nuntius) are hackathon-grade, 0 to 1 stars, TypeScript, and mostly built on their own contracts rather than the official program.

**Non-goals.** This engine does not re-implement authorization, custody funds, replace the on-chain program, or act as a payment processor of record.

## Design principles

**The chain owns authorization; the engine owns operation.** Never duplicate what the program already enforces (caps, cadence, who may pull). Own only what it cannot: timing, failure handling, and recovery.

**Non-custodial by construction.** Two one-time setup signatures precede everything: the merchant publishes the plan (price, cadence, permitted pullers), and the subscriber signs the SPL approval granting capped pull authority. After that, each cycle's charge needs no live signature from either party. Funds move directly from subscriber to merchant; the engine is never in the money path.

The worst a compromised puller key can do is trigger a legitimate, in-cap charge to the correct merchant earlier than scheduled. It cannot exceed the cap, redirect funds, or withdraw.

**Two deployment shapes, one codebase.** Self-hosted (operator supplies their own puller key) is the primary path and the open-source deliverable. Hosted (the service holds an authorized puller key) is a configuration of the same engine, not a separate product. They differ by signer backend only.

**Open-core split.** The grant funds the engine: library, self-host daemon, tests, docs. It must be fully usable standalone: clone, point at any RPC, run. The hosted instance competes on convenience (uptime, dashboard, support), never by withholding core functionality. Server costs and product work sit outside the grant line.

## Architecture: complete version

&#91;embedded content: keeper architecture · 8 components, 1 chain boundary\]

The sync worker reads chain state; only the submitter writes to it. Everything in between is local decision-making.

**Sync worker.** Reconciles program accounts (plans, delegations, subscriptions) into local state. Detects new subscribers, cancellations, plan changes. The chain is truth for what is authorized; the local store is truth for what has been attempted.

**Scheduler.** Decides what is due now, emits work items, and leases them so two instances cannot charge the same period.

**Pre-flight checker.** Before spending SOL on a transaction, verify collectability: delegate still set, remaining cap sufficient, token account not frozen or closed, balance adequate. Turns most failures into classifications instead of burned fees.

**Submitter.** Builds, signs, sends, confirms. Uses a durable nonce rather than a recent blockhash, so a pending charge stays resumable across restarts instead of silently expiring.

**Outcome classifier.** Sorts every result into four classes; all downstream behaviour keys off this.

**Dunning state machine.** Per-merchant backoff and grace windows. This is the part merchants will not write themselves.

**Signer backend.** One interface over local keypair, env var, KMS, or external signer. Self-host and hosted differ here and nowhere else.

**Event emitter and reconciliation.** Signed webhooks on every state change, plus an owed-versus-collected report per period with failure rates by class.

## Data model and the exactly-once invariant

The database write and the chain submission are not atomic. A naive keeper that increments a local counter after sending will eventually double-charge someone, or skip a period after a crash.

Two mechanisms prevent it:

1. **Write intent before sending.** A charge intent row (subscription, period key, durable nonce, signature, status) is persisted *before* the transaction goes out. A crash between write and confirmation leaves a recoverable record, not a mystery.
2. **Derive idempotency from chain state.** The delegation PDA's last-pull timestamp is the authoritative answer to "was this period already collected?" On restart, reconcile every pending intent against the chain before retrying anything. Local state is a cache of attempts, never the source of truth about money moved.

**Core tables.**

| Table | Holds | Truth source |
| --- | --- | --- |
| `subscription` | merchant, subscriber, mint, cadence, amount, chain status | chain, synced |
| `charge_intent` | period key, nonce, signature, attempt count, status | local |
| `charge_outcome` | result class, program error, slot, confirmed amount | chain, recorded |
| `dunning_state` | current state, retries used, grace expiry, next action | local |
| `webhook_delivery` | event, endpoint, attempts, last response | local |

The period key is the idempotency key: one successful charge per `(subscription, period)` pair, enforced by a unique constraint and validated against the chain before every send. The period is not ours to define. The program fixes it: a stride of `period_length_s` anchored at `current_period_start_ts`, advanced in whole multiples and floored, never calendar-aligned and never rolled from the last charge. The keeper derives the current period as `floor((now − anchor) / period_length)` from on-chain state and reconciles against it rather than inventing period semantics.

## Failure handling and dunning

Classification is the engine's core judgement: the same failed transaction means "try again in 30 seconds" or "this customer has left" depending on why it failed.

**Transient** failures retry the same period with exponential backoff and a per-period attempt cap, so a congested hour does not burn the retry budget meant for real problems.

**Hard** failures move the subscription into dunning rather than retrying immediately. A subscriber with an empty account will not gain funds in 30 seconds, and repeated doomed sends waste SOL. The pre-flight checker catches most of these before any transaction is built.

**Terminal** outcomes stop scheduling entirely. A cancelled subscription or removed plan is not a failure to recover from.

The grace window, retry budget and notification cadence are merchant-level configuration. A SaaS tool may suspend after 3 days; a payroll operator may never suspend and only alert. One hard bound from the chain: because a missed charge becomes unrecoverable once the period boundary passes (the cap resets and skipped periods do not stack), the retry-and-grace window for any single charge cannot usefully exceed its own period length. This also settles catch-up after downtime. Multi-period catch-up is impossible, so an outage spanning a full period is lost revenue, not deferred; on restart the keeper attempts only the current open period's charge, never a backlog.

## Design decisions & trade-offs

Each is a deliberate trade-off with a default already chosen; the Phase 0 spike confirms them before the engine is built.

**1. Signer custody in hosted mode.** Local keypair for self-host is settled. For the hosted path, whether the puller key lives in KMS or an external signer service changes the deployment story but not the engine. Defer the choice, keep the interface.

**2. RPC strategy.** Polling program accounts versus subscribing to logs. Polling is simpler and survives dropped sockets; subscriptions cut latency. Latency does not matter much for monthly billing, which argues for polling in v0.

## MVP scope

v0 proves one thing: the engine reliably triggers pulls against the real program and survives failures. Everything else waits.

**In scope.** Single instance, no leasing. Postgres. CLI daemon with a config file and a local keypair. Recurring delegations only. Sync worker, scheduler, pre-flight checker, submitter, classifier, dunning state machine, plain POST webhooks. Devnet.

The dunning logic stays in v0 even though it is the largest piece, because it *is* the differentiator. A keeper without it is a cron job.

**Out of scope.** Hosted API, dashboard, multi-tenancy, KMS and external signers, high availability and leasing, allowances and merchant-plan flows, Token-2022 extension edge cases, notification templates, mainnet.

**Proof of work.** The deliverable that matters most is the forced-failure harness: set up a subscription on devnet, deliberately freeze or empty the subscriber's token account mid-cycle, and show the engine detect it, classify it as hard, skip pointless retries, and move the subscription into dunning with a grace window, rather than looping or double-charging on restart. A linked devnet transaction plus that failing-case test is stronger evidence than any write-up.

A second test covers crash recovery: kill the process between intent write and confirmation, restart, and show it reconcile against the chain without re-charging the period.

## Milestones

The Solana Foundation Brazil track pays 25% on approval and the rest against self-described milestones. Scope each to a demonstrable artifact.

| # | Deliverable | Demonstrates |
| --- | --- | --- |
| 1 | Core loop: sync, schedule, submit, confirm on devnet | the keeper works against the official program |
| 2 | Pre-flight checker, classifier, dunning state machine | the operational layer nobody has shipped |
| 3 | Crash recovery, forced-failure harness, test suite | exactly-once per period, proven |
| 4 | Self-host daemon, config, docs, reconciliation report | anyone can run it without the author |

The hosted product sits outside this line. It is mentioned in the application as the adoption path, never as a funded line item.
