# Accounting implementation provenance

Token Monitor deliberately does **not** maintain an independent implementation
of AI-client token accounting. The project keeps a small serverless transport
and dashboard layer around mature upstream accounting code.

## Source-of-truth map

| Concern | Source of truth | Token Monitor policy |
| --- | --- | --- |
| Client discovery and session parsing | `junhoyeo/tokscale` v4.14.0 / `tokscale-core` | Use the library directly. Do not reimplement client parsers. |
| Codex cumulative/delta handling and fork/dedup rules | `tokscale-core::sessions::codex` | Use Tokscale output as the canonical total. |
| Claude/OpenCode/Gemini/Kimi/Cursor/etc. parsing | Tokscale client modules | Use Tokscale output as the canonical total. |
| Model canonicalization | `tokscale_core::canonical_model_id` | Use directly before persistence. |
| General provider-aware API-equivalent pricing | `tokscale_core::pricing::PricingService` | Use directly; do not maintain a duplicate general model rate table. |
| Additive token buckets | Tokscale `TokenBreakdown` semantics | Preserve input/output/cache-read/cache-write/reasoning semantics. |
| Codex service-tier evidence | Adapted from `falyx6851-byte/codex-monitor` | Enhancement only. It may subdivide canonical Codex totals only after a reconciliation check. |
| Codex Fast/Standard pricing rules | Adapted from `falyx6851-byte/codex-monitor` | Logic is ported; rate data is refreshed from current official OpenAI sources. |
| Route classification | Token Monitor extension | Keep upstream model vendor separate from actual route provider; never infer `official` from model name alone. |
| Device sync/encryption/Pages | Token Monitor | Project-specific implementation. |

## Pinned upstream

Tokscale dependency:

- release: `v4.14.0`
- commit: `1ec865d4e9e5adf157efb9f5c2dfaf29630d71e5`
- license: MIT

CI checks out the same release and runs the complete `tokscale-core`
parser/scanner test suite in addition to this repository's tests.

## Why Codex has a separate enhancement path

Tokscale remains the canonical source for Codex token totals because its parser
handles cumulative counters, resets, forks, archived-session duplicates,
reasoning/output normalization, and source caching. The optional Codex tier
adapter exists only because service-tier evidence (`standard`, `fast`, etc.) is
not part of Tokscale's public `ParsedMessage` shape.

The adapter follows the request-level state-machine and pricing behavior from
`falyx6851-byte/codex-monitor`. It is not allowed to silently replace Tokscale:
its additive token totals must reconcile with the Tokscale total for the same
bucket before tier-specific breakdowns are accepted. A mismatch degrades to the
Tokscale row with the tier marked unavailable.

## Price-data policy

Pricing is versioned data, not code. Historical `pricing.json` files from other
projects are **not** copied blindly. Any locally stored tier-rate table must
contain:

- `updated_at`
- official source URLs
- explicit model/tier/context keys
- tests for known examples

Unknown or unsupported price combinations are left unpriced instead of being
inferred from nearby models or tiers.

## Privacy boundary

Upstream parsers read local session data. Token Monitor persists only normalized
aggregate statistics and routing labels into the encrypted ledger. Prompts,
responses, reasoning text, source files, project paths, full transcripts and
credentials are not part of the ledger schema.
