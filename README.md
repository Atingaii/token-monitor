<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Atingaii/token-monitor/actions/workflows/rust-cli-ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Atingaii/token-monitor/rust-cli-ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://github.com/Atingaii/token-monitor/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Atingaii/token-monitor?style=flat-square&display_name=tag"></a>
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-000000?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555?style=flat-square">
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2f81f7?style=flat-square"></a>
</p>

<p align="center">
  <strong>One lightweight CLI. Multiple AI coding clients. Every device in one encrypted analytics dashboard.</strong>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="https://atingaii.github.io/token-monitor/">Dashboard</a> ·
  <a href="SOURCES.md">Accounting Sources</a> ·
  <a href="https://github.com/Atingaii/token-monitor/releases">Releases</a>
</p>

## What is Token Monitor?

Token Monitor is a **serverless, cross-device, CLI-only usage analytics system for AI coding tools**.

It uses the pinned **Tokscale Core v4.14.0** for mature client discovery, parsing, deduplication, token semantics and model normalization. Token Monitor adds route evidence, subscription-equivalent pricing, encrypted cross-device snapshots and a static GitHub Pages dashboard.

There is no Electron runtime, Node daemon, Python service, Docker stack, VPS, telemetry database or always-on hub to operate.

## Highlights

| Capability | Implementation |
| --- | --- |
| Multi-client accounting | Reuses Tokscale v4.14.0 parsers/scanners instead of maintaining duplicate client parsers |
| Cross-device | Windows, Linux and macOS on x64 and ARM64 |
| Zero resident process | Native OS schedulers launch a short `sync --quiet` run and exit |
| Encrypted snapshots | AES-256-GCM device ledgers in per-device `tm-ledger-*` branches |
| Memorable dashboard access | No long `#key=...` URL; the same password unlocks the workspace from any browser |
| Route provenance | Model vendor, route provider, route type and raw provider remain separate dimensions |
| Canonical official route | Proven first-party routes use one `official` route-provider bucket; raw provider evidence remains available |
| Mature pricing | General catalog follows CC Switch's models.dev source; GPT-5.6 is guarded to CC Switch/Sub2API-compatible subscription-equivalent rates |
| Precise charts | Readable K/M/B/T axes plus exact hover/table values and adaptive labels |
| Idle-write suppression | No GitHub snapshot write when accounting has not changed |

## Architecture

```text
Windows / Linux / macOS
        │
        │ local AI coding usage
        ▼
Tokscale Core v4.14.0
parser · dedup · token semantics · model normalization
        │
        ▼
Token Monitor
route evidence · subscription-equivalent pricing
        │
        ├── AES-256-GCM device ledger
        ▼
YOUR_NAME/token-monitor
├── main                       source code
├── tm-dashboard               password-wrapped workspace key only
├── tm-ledger-<device-A>       encrypted ledger.json
├── tm-ledger-<device-B>       encrypted ledger.json
└── tm-ledger-<device-C>       encrypted ledger.json
        │
        ▼
Atingaii GitHub Pages
password → local key unwrap → local ledger decrypt → analytics
```

Telemetry never lands on `main`. Each device force-moves only its own history-free snapshot branch.

## Install

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

Linux v1.1+ artifacts are built as **static musl binaries**. Release CI rejects Linux artifacts that still contain a dynamic ELF interpreter or glibc symbol-version dependency, avoiding the old `GLIBC_2.xx not found` failure on older distributions.

Because `curl | sh` runs in a child shell, it cannot mutate the parent shell's PATH. The installer prefers an already-active user bin directory. Otherwise it prints an absolute setup command that works immediately, for example:

```bash
'/home/you/.local/bin/token-monitor' setup
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

Windows PowerShell 5.1 and PowerShell 7 are supported. The installer includes TLS 1.2 compatibility, native x64/ARM64 detection, SHA-256 verification and PATH fallback handling.

> Do **not** use the Unix `curl -fsSL ... | sh` command in Windows PowerShell. In Windows PowerShell, `curl` commonly aliases `Invoke-WebRequest`, which does not understand Unix curl's `-fsSL` flags.

## First device

```bash
token-monitor setup
```

Setup automatically:

1. resolves a GitHub write credential;
2. finds or creates your Token Monitor fork;
3. generates a random 256-bit workspace key;
4. asks for a memorable dashboard password using hidden terminal input;
5. wraps the workspace key with PBKDF2-HMAC-SHA256 + AES-256-GCM and publishes only the wrapped manifest to `tm-dashboard/access.json`;
6. performs the first full local scan and encrypted snapshot upload;
7. installs the native low-load scheduler;
8. prints the stable Dashboard URL and another-device join command.

The Dashboard URL is now stable:

```text
https://atingaii.github.io/token-monitor/?repo=YOUR_NAME/token-monitor
```

There is no encryption key in the URL. A new browser, another machine or a private window only needs the same dashboard password.

## Add another device

The first device prints:

```bash
token-monitor join '<PAIR_CODE>'
```

The pair code contains the repository, workspace key and sync cadence, but **not** the GitHub token. Treat it as sensitive workspace material.

Print it again with:

```bash
token-monitor invite
```

## Dashboard password model

```text
memorable password
  │ PBKDF2-HMAC-SHA256 / 310,000 iterations / random salt
  ▼
wrapping key
  │ AES-256-GCM
  ▼
random workspace key
  │ AES-256-GCM
  ▼
per-device ledger.json
```

GitHub stores only the salt, nonce, iteration count and ciphertext. The password is not uploaded and never appears in the URL. The browser derives the wrapping key and decrypts ledgers locally.

This remains a static-site architecture, so an attacker can download `access.json` and attempt offline password guessing. Use a longer memorable passphrase rather than a weak numeric password.

Change the password with:

```bash
token-monitor password
```

Changing the password re-wraps the same workspace key; device ledgers do not need to be rewritten.

## Upgrade from v1.0

Re-run the platform installer, then on any already-configured device run:

```bash
token-monitor password
token-monitor sync
```

v1.1 detects the older ledger/pricing schema, discards the stale local cache for accounting purposes and automatically performs **one full rescan + full historical reprice**. Later runs return to the normal two-day incremental overlap. This prevents a mixed ledger where recent days use the new rate card but older history keeps v1.0 prices.

Legacy `#key=...` URLs remain readable for migration, but v1.1 no longer emits them.

## Pricing policy

The dashboard reports **subscription-equivalent cost**, not an invoice and not the actual amount deducted by a ChatGPT/Codex subscription.

### GPT-5.6

GPT-5.6 uses CC Switch's current built-in rate card, independently cross-checked against Sub2API's fallback values. USD per 1M tokens:

| Model | Input | Cache Read | Cache Write | Output |
| --- | ---: | ---: | ---: | ---: |
| GPT-5.6 Sol | **$5.00** | **$0.50** | **$6.25** | **$30.00** |
| GPT-5.6 Terra | $2.00 | $0.20 | $2.50 | $12.00 |
| GPT-5.6 Luna | $0.20 | $0.02 | $0.25 | $1.20 |

GPT-5.6 Fast/Priority uses explicit **2×** rates. The `>272K` long-context decision is made at **request granularity before aggregation**, so a multi-million-token day is never mistaken for one long-context request.

Example:

```text
182,000 fresh input × $5/M
6,080,000 cache read × $0.50/M
12,000 output × $30/M
≈ $4.31
```

That is why the prior API-like `$4/$20/$0.40` card produced roughly `$3.40`, while the v1.1 CC Switch-compatible subscription-equivalent card produces roughly `$4.31` for the same usage.

### Other models

The general catalog follows the same public source CC Switch can sync:

```text
https://models.dev/api.json
```

If the same normalized model appears under several providers, Token Monitor deterministically prefers the model family's canonical provider. Missing billable buckets are marked as lower-bound pricing rather than guessed from a neighboring model.

See [`SOURCES.md`](SOURCES.md) for exact provenance and responsibilities.

## Route semantics

| Field | Meaning | Example |
| --- | --- | --- |
| `model` | Canonical model | `gpt-5.6-sol` |
| `upstreamVendor` | Model owner/family | `openai` |
| `routeProvider` | Actual normalized route supplier | `official`, `azure-openai`, `openrouter`, `newapi` |
| `routeType` | Route class | `official`, `cloud`, `aggregator`, `relay`, `unknown` |
| `provider` | Raw source provider | retained for auditing |

When route evidence proves a first-party OpenAI/Anthropic/Google/etc. route, `routeProvider` is normalized to `official`. The vendor remains in `upstreamVendor`, so model ownership is not lost. Seeing a GPT or Claude model alone is **not** sufficient evidence to call the route official.

## Dashboard

v1.1 uses a modern, restrained admin-console layout: neutral surfaces, light borders, a controlled blue accent, compact filters and a collapsible sidebar. It is visually inspired by the information density and layout direction of current New API, but Token Monitor does not copy New API's AGPL implementation.

Features include:

- Today / 7d / 30d / current month / all / custom date range
- device, client, model, model vendor, route provider, route type, raw provider and tier filters
- line, area, bar, stacked bar, donut, treemap and table views
- CSV export
- light/dark themes
- collapsible desktop sidebar and mobile drawer
- adaptive Y-axis nice scaling and left margin
- readable K/M/B/T axis labels with full exact values in tooltips
- adaptive X-axis label sampling/rotation
- full comma-separated integer token values in tables and four-decimal cost detail

## Background load

| Platform | Scheduler |
| --- | --- |
| Windows | Task Scheduler |
| macOS | launchd |
| Linux | `systemd --user` timer with cron fallback |

Default cadence is 15 minutes. No Token Monitor process remains resident. Unchanged accounting snapshots skip the GitHub write entirely.

## Privacy boundary

Encrypted ledgers may contain:

- date and device identity
- client/model/provider/route/tier labels
- input/cache-read/cache-write/output/reasoning token buckets
- additive record count
- subscription-equivalent cost and lower-bound flags

They do **not** upload prompts, assistant responses, reasoning text, source code, project contents, project paths, full JSONL/SQLite sessions, `auth.json`, API keys or GitHub tokens.

## Commands

```text
token-monitor setup [--repo OWNER/REPO]
token-monitor join <PAIR_CODE>
token-monitor password
token-monitor invite
token-monitor sync [--full]
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor uninstall [--remove-remote] [--purge]
```

## Release gate

A release must pass more than a Linux compile:

- Linux x86_64 musl
- Linux ARM64 musl
- Windows x86_64
- Windows ARM64
- macOS Intel
- macOS Apple Silicon
- Rust clippy
- dashboard JS / analytics / privacy regressions
- pinned Tokscale v4.14.0 parser/scanner regression suite
- post-publish install from the real GitHub Release assets
- Windows PowerShell 7 and **Windows PowerShell 5.1** install smoke tests
- Linux ELF static-link / GLIBC symbol-version guards

## Provenance and license

Token Monitor is MIT licensed. Mature implementation sources and adaptations are documented in [`NOTICE`](NOTICE) and [`SOURCES.md`](SOURCES.md), including:

- `Javis603/token-monitor`
- `junhoyeo/tokscale` v4.14.0
- `falyx6851-byte/codex-monitor`
- CC Switch's models.dev pricing synchronization approach and GPT-5.6 rate reference
- Sub2API's LiteLLM pricing source and GPT-5.6 fallback cross-check

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`SECURITY.md`](SECURITY.md). For client parsing or token accounting, prefer fixing/upgrading the mature upstream engine rather than adding an unverified duplicate parser inside Token Monitor.
