# Contributing to Token Monitor

Thanks for helping improve Token Monitor.

## Before you start

Token Monitor intentionally keeps its own accounting logic thin. The following rules are project invariants, not style preferences:

1. **Do not add a second hand-written client parser when Tokscale already supports the client.** Prefer updating the pinned Tokscale version or contributing the fix upstream.
2. **Do not duplicate generic pricing logic.** Generic token pricing belongs to Tokscale. A specialized adapter is acceptable only when the upstream accounting model cannot represent the required source evidence, and it must fail closed.
3. **Preserve provenance.** Any imported or adapted accounting logic must identify its upstream repository/version/license in `SOURCES.md` and `NOTICE` when applicable.
4. **Never infer an official route from model family alone.** Provider/route classification must be based on source evidence.
5. **Never upload raw session content.** GitHub ledgers may contain normalized numeric aggregates and routing labels only.
6. **Cross-device metrics must be additive.** Distinct session counts, ambiguous duration measures, and other non-additive values do not belong in the global ledger totals.

## Development setup

Install a current Rust toolchain, then run:

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets
cargo build --release --workspace
```

Dashboard checks can be run with Node.js when contributing to `web/`:

```bash
node --check web/analytics.js
node --check web/app.js
node web/analytics.test.cjs
```

Node.js is a contributor/testing convenience only; it is not a runtime dependency for end users.

## Pull requests

A good PR should include:

- a focused problem statement;
- tests for changed behavior;
- accounting provenance when touching token semantics, pricing, model normalization, or client parsing;
- Windows/Linux/macOS considerations for runtime changes;
- privacy impact for new uploaded fields;
- documentation changes when CLI or ledger behavior changes.

## Accounting changes

For changes involving client support, token semantics, deduplication, model normalization, or generic price calculation:

1. Check whether the issue is already fixed upstream in Tokscale.
2. Prefer an upstream Tokscale fix and pin/update to a reviewed release.
3. Run the pinned Tokscale parser/scanner regression suite in CI.
4. If a narrow local adapter is unavoidable, reconcile it against canonical Tokscale accounting before accepting its output.

Codex service-tier handling is the current example of such an adapter: it may refine a reconciled canonical day, but must never silently replace canonical usage when totals differ.

## Provider routing changes

Keep these concepts distinct:

- model identity;
- upstream model vendor;
- raw provider identifier;
- evidenced route provider;
- route type.

Unknown evidence should stay unknown. Accuracy is more important than filling every dashboard field.

## Privacy review

Before adding a ledger field, ask whether it can expose prompt text, response text, source code, project names/paths, credentials, or other user content. If yes, it does not belong in the remote ledger.

## CI gate

Do not treat one-platform compilation as release evidence. The intended gate includes Linux x64/ARM64, Windows x64/ARM64, macOS Intel/Apple Silicon, Rust quality checks, dashboard tests, and the pinned Tokscale regression suite.

## License

By contributing, you agree that your contribution is licensed under the repository's MIT License. Preserve upstream copyright and license notices for adapted code.
