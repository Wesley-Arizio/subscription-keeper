# subscription-keeper

An open-source Rust keeper and dunning engine for recurring stablecoin payments on Solana.

## What it is

Solana's official [Subscriptions & Allowances program](https://github.com/solana-program/subscriptions) authorizes recurring pulls but never triggers them, and Solana has no native scheduler. Someone has to submit every charge, every cycle, then handle what happens when a charge fails. Today every integrator rebuilds that operational layer in their own closed stack.

`subscription-keeper` is that layer, built once and in the open. It watches on-chain subscriptions, decides what is due, submits each charge against the official program, and runs the retry, grace, and dunning logic that turns a raw "pull" primitive into something a merchant can actually bill on.

## Principles

- **The chain owns authorization; the engine owns operation.** It never duplicates the program's caps or cadence, only the timing, failure handling, and recovery the program leaves to you.
- **Non-custodial by construction.** The merchant publishes a plan and the subscriber signs a capped approval once; after that, funds move directly from subscriber to merchant and the engine is never in the money path.
- **One codebase, two deployments.** Self-host (you supply the puller key) is the primary, open-source path. A hosted instance is the same engine with a different signer backend.

## Status

Early. The design is settled and the v0 build is next: a single-instance daemon on Postgres that reliably triggers pulls on devnet and survives induced failures. See the design doc for scope and milestones.

## Design

Full architecture, data model, failure handling, and open trade-offs live in [`DESIGN.md`](./DESIGN.md).

## License

TBD (will be OSI-approved open source).