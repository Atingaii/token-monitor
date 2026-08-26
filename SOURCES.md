# Accounting implementation provenance

Token Monitor deliberately does **not** maintain a second hand-written implementation of AI-client token accounting. The project adds a small serverless sync, route-provenance and dashboard layer around mature upstream accounting code.

## Source-of-truth map

| Concern | Source of truth | Token Monitor policy |
| --- | --- | --- |
| Client discovery and session parsing | `junhoyeo/tokscale` v4.14.0 / `tokscale-core` | Use the library directly. Do not reimplement client parsers. |
| Codex cumulative/delta handling, resets, forks and dedup | `tokscale-core::sessions::codex` | Tokscale totals are canonical. |
| Claude/OpenCode/Gemini/Kimi/Cursor/etc. parsing | Tokscale client modules | Tokscale output is canonical. |
| Model canonicalization | `tokscale_core::canonical_model_id` | Use directly before persistence. |
| General provider-aware **API-equivalent** pricing | `tokscale_core::pricing::PricingService` | Use directly; no duplicate general model price table. |
| Additive token buckets | Tokscale `TokenBreakdown` semantics | Preserve input/output/cache-read/cache-write/reasoning semantics. |
| Codex service-tier evidence | Adapted from `falyx6851-byte/codex-monitor` | May subdivide canonical Codex totals only after exact day-level token + message reconciliation. |
| Codex current API Fast/Standard estimate | Adapted parser/formula + current official OpenAI rates | Kept separate from plan-equivalent metering. |
| Codex included-plan / legacy-meter equivalent | Official OpenAI rate-card/launch statements + tier evidence | Planning estimate only; never presented as an invoice. |
| Route classification | Token Monitor extension | Upstream model vendor is distinct from route provider; model name alone never proves `official`. |
| Device sync, public sanitization, encrypted compatibility file and Pages | Token Monitor | Project-specific implementation with regression tests. |

## Pinned upstream accounting engine

Tokscale dependency:

- release: `v4.14.0`
- commit: `1ec865d4e9e5adf157efb9f5c2dfaf29630d71e5`
- license: MIT

CI checks out the same release and runs the complete `tokscale-core` parser/scanner test suite in addition to Token Monitor's own tests.

## Why Codex has a separate tier adapter

Tokscale remains canonical for Codex token totals because its parser handles cumulative counters, resets, forks, archived-session duplicates, reasoning/output normalization and scanner caching. Service-tier evidence (`standard`, `priority` / `fast`) is not part of the public Tokscale `ParsedMessage` shape, so Token Monitor uses a narrow adapter based on `falyx6851-byte/codex-monitor` to recover request-level tier evidence.

The adapter cannot redefine usage. Its day-level additive token total **and** normalized message count must equal Tokscale for the same day before Fast/Standard rows replace the generic Codex rows. Pricing completeness is not itself a reconciliation requirement: if token accounting matches but a billable component is unavailable, the tier is retained and the affected cost is explicitly marked as a lower bound.

## Two cost bases

The dashboard intentionally exposes two different estimates because OpenAI's current GPT-5.6 Sol promotional API/token-billed price and included-plan/legacy metering are not the same concept.

### Current API-equivalent (`costUsd`)

- General models: Tokscale `PricingService`.
- GPT-5.6 Codex rows with reconciled tier: current official OpenAI API / Fast-mode rate data.
- This answers: *"what would this observed token shape be worth at the current API-style rate?"*

### Included-plan / legacy-meter equivalent (`planCostUsd`)

Only rows whose Codex tier can be independently reconstructed receive this value. For GPT-5.6 Sol, the planning basis preserves the original Sol launch rate where OpenAI states that the later promotional reduction does **not** change included plan usage, 5-hour or weekly limits, or legacy credit rates. Fast-mode planning uses the applicable Codex/Work rate-card multiplier on that plan basis.

This answers: *"what is a useful dollar-equivalent planning value for the unchanged plan/legacy meter?"* It is **not** a statement that a Plus/Pro subscription was invoiced per token.

Terra/Luna plan-equivalent bases follow the reduced paid-subscription consumption where OpenAI explicitly states those reductions are reflected in paid subscriptions.

### Official rate references

- OpenAI API pricing: https://developers.openai.com/api/docs/pricing
- OpenAI API Fast mode: https://openai.com/api-fast-mode/
- ChatGPT token/credit rate card and GPT-5.6 Sol promotional-note language: https://help.openai.com/en/articles/20001415
- GPT-5.6 Sol launch pricing / cache-write multiplier: https://openai.com/index/previewing-gpt-5-6-sol/

The versioned Codex rate data lives in `rust-cli/pricing/codex_tiers.json` and records these sources in the file itself.

## Unknown-price policy

Unknown or unsupported model/tier combinations are **not guessed**. Numeric known components may be retained as a lower bound; the row is flagged accordingly. Token totals remain independent of price coverage.

## Public dashboard boundary

From v1.0.1 each history-free device branch contains:

- `ledger.json`: AES-256-GCM encrypted compatibility aggregate;
- `public.json`: intentionally public, de-identified aggregate used by GitHub Pages.

`public.json` never contains prompt/response text, reasoning text, source code, project paths, session IDs, raw device IDs, configured host/device names or credentials. It exposes only the additive aggregate dimensions required by the dashboard plus an anonymous device hash/label. See `SECURITY.md` for the exact boundary.

## Attribution

- `junhoyeo/tokscale` — MIT, Copyright 2025 Junho Yeo.
- `falyx6851-byte/codex-monitor` — MIT, Copyright 2026 falyx6851-byte.
- Original project lineage: `Javis603/token-monitor`.

See `NOTICE` for retained notices.
