# Accounting implementation provenance

Token Monitor deliberately avoids inventing client token semantics or model
rates. Its local parser, tier evidence, price catalog, and special-rate policies
are tied to mature upstream implementations and explicit version/data sources.

## Source-of-truth map

| Concern | Source of truth | Token Monitor policy |
| --- | --- | --- |
| Client discovery and session parsing | `junhoyeo/tokscale` v4.14.0 / `tokscale-core` | Use the library directly. Do not reimplement client parsers. |
| Codex cumulative/delta handling and fork/dedup rules | `tokscale-core::sessions::codex` | Tokscale is the canonical additive token total. |
| Claude/OpenCode/Gemini/Kimi/Cursor/etc. parsing | Tokscale client modules | Preserve upstream-normalized token buckets. |
| Model canonicalization | `tokscale_core::canonical_model_id` | Use directly before persistence. |
| Additive token buckets | Tokscale `ParsedMessage` / `TokenBreakdown` semantics | Cache-read is not double-billed as fresh input; reasoning remains a separate additive bucket. |
| General model price catalog | CC Switch (`farion1231/cc-switch`) models.dev integration | Read the same public `https://models.dev/api.json` catalog and normalize model IDs using the same public schema. |
| Cost arithmetic | CC Switch usage calculator | Price normalized fresh input, cache read, cache creation and output independently. Do not recreate raw-client token semantics. |
| GPT-5.6 guarded rates | CC Switch built-in seed table; independently cross-checked against Sub2API fallback data | Keep Sol/Terra/Luna and Fast/Priority rates stable even if a generic API catalog changes. |
| GPT-5.6 long-context policy | CC Switch/Sub2API-compatible request billing | Evaluate >272K input-side tokens per request; input/cache-read/cache-write use 2x and output uses 1.5x. |
| Codex service-tier evidence | Adapted from MIT-licensed `falyx6851-byte/codex-monitor` | Tier attribution only; it cannot change the canonical Tokscale token total. |
| Route classification | Token Monitor extension | Keep upstream model vendor separate from actual route provider; never infer `official` from model name alone. |
| Device sync/encryption/password wrapping/Pages | Token Monitor | Project-specific transport and UI implementation. |

## Pinned parser upstream

Tokscale dependency:

- release: `v4.14.0`
- commit: `1ec865d4e9e5adf157efb9f5c2dfaf29630d71e5`
- license: MIT

CI checks out the same release and runs the complete `tokscale-core`
parser/scanner test suite in addition to this repository's own tests.

## Pricing source: CC Switch + models.dev

Primary mature implementation reference:

- repository: `farion1231/cc-switch`
- license: MIT
- public catalog: `https://models.dev/api.json`

CC Switch treats models.dev prices as USD per million tokens and keeps input,
output, cache read and cache write as distinct billing buckets. Token Monitor
follows that model after Tokscale has normalized the raw client counters.

For GPT-5.6, Token Monitor intentionally uses the guarded CC Switch seed rates
instead of allowing the generic catalog to silently change the user's
subscription-equivalent accounting policy:

| Model | Input / MTok | Output / MTok | Cache read / MTok | Cache write / MTok |
| --- | ---: | ---: | ---: | ---: |
| `gpt-5.6-sol` | $5.00 | $30.00 | $0.50 | $6.25 |
| `gpt-5.6-terra` | $2.00 | $12.00 | $0.20 | $2.50 |
| `gpt-5.6-luna` | $0.20 | $1.20 | $0.02 | $0.25 |

For these GPT-5.6 variants, `fast` / `priority` uses the explicit 2x price card.
Long-context billing is evaluated at request granularity when total input-side
tokens exceed 272,000; input/cache-read/cache-write use a 2x multiplier and
output uses 1.5x.

Sub2API (`Wei-Shaw/sub2api`, LGPL-3.0) independently contains the same GPT-5.6
fallback values and long-context policy. Token Monitor uses that project only as
an independent cross-check of the numeric policy; no Sub2API source code is
copied or linked into Token Monitor.

Unknown models or missing billable buckets are not guessed. They remain
unpriced or are explicitly marked as a lower-bound cost.

## Why Codex has a separate tier-evidence path

Tokscale remains the canonical source for Codex token totals because its parser
handles cumulative counters, resets, forks, archived-session duplicates,
reasoning/output normalization, and source caching. The narrow Codex adapter
exists only because request-level service-tier evidence (`standard`, `fast`) is
not part of Tokscale's public `ParsedMessage` shape.

The adapter follows the explicit tier-state/request-usage behavior from
`falyx6851-byte/codex-monitor`. A day's tier breakdown is accepted only when its
**additive token total exactly equals Tokscale for that day**. Request/message
record counts do not gate the split because the two mature parsers intentionally
expose different record granularities. A token mismatch always falls back to
canonical Tokscale rows.

Tier rows remain request-granular until after pricing so a 272K long-context
threshold is never incorrectly applied to a whole day's aggregate.

## Dashboard password boundary

The random 256-bit workspace ledger key still encrypts every device snapshot.
A user-chosen dashboard password does not replace that key and is never stored.
Instead, PBKDF2-HMAC-SHA256 derives a local wrapping key and AES-256-GCM wraps
the existing workspace key into `tm-dashboard/access.json`. Any browser can
perform the same local unwrap after the user enters the password.

This is a static-site convenience/privacy barrier, not a server-side login
service. Because the public password-wrapped envelope can be downloaded for
offline guessing, users should choose a strong passphrase rather than a trivial
password.

## Privacy boundary

Upstream parsers read local session data. Token Monitor persists only normalized
aggregate statistics and routing labels into encrypted ledgers. Prompts,
responses, reasoning text, source files, project paths, full transcripts and
credentials are not part of the ledger schema.
