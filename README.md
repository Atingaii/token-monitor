<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor — serverless cross-device usage analytics for AI coding tools" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Atingaii/token-monitor/actions/workflows/rust-cli-ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Atingaii/token-monitor/rust-cli-ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://github.com/Atingaii/token-monitor/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Atingaii/token-monitor?style=flat-square&display_name=tag"></a>
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-000000?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555?style=flat-square">
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2f81f7?style=flat-square"></a>
</p>

<p align="center">
  <strong>One lightweight CLI. Every device. Every supported AI coding client. One private-by-design analytics view.</strong>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#dashboard">Dashboard</a> ·
  <a href="SOURCES.md">Accounting Sources</a>
</p>

Token Monitor is a **serverless, cross-device usage analytics system for AI coding tools**. It installs as one prebuilt Rust executable on Windows, Linux, or macOS, reads local usage through the pinned Tokscale Core accounting engine, encrypts normalized device snapshots, and stores them in the user's own Token Monitor fork. A static GitHub Pages dashboard reads those encrypted snapshots and decrypts them locally in the browser.

There is no Electron app, Node.js runtime, Python environment, Docker stack, VPS, database, always-on hub, or second data repository to operate.

## Why Token Monitor

Most usage monitors answer *“what did this machine use?”*. Token Monitor is designed to answer the harder cross-device questions:

- How many tokens did all of my machines use together?
- Which device, client, model, provider route, or service tier consumed them?
- Was a GPT/Claude request routed through an official API, a cloud provider, OpenRouter, or a relay?
- What is the API-equivalent value of the observed usage?
- Can I keep the accounting layer lightweight without running a permanent monitoring service?

## Core principles

| Principle | Implementation |
| --- | --- |
| **Low overhead** | No resident daemon. Native OS schedulers launch a short one-shot sync and exit. |
| **Cross platform** | Native release/test targets for Windows, Linux, and macOS on x64 and ARM64. |
| **Mature accounting core** | Client parsing, token semantics, deduplication, model normalization, and generic pricing come from pinned **Tokscale v4.14.0**. |
| **Evidence-first routing** | Model vendor and actual request route are separate. A GPT model alone does not prove “OpenAI official”. |
| **Private by design** | Only aggregate numeric usage and routing labels are uploaded, encrypted with AES-256-GCM. |
| **No telemetry database** | Each device owns one encrypted `tm-ledger-*` branch in the user's fork. |
| **Fail closed on special accounting** | Codex Fast/Standard enhancement is accepted only when it reconciles exactly with canonical Tokscale daily totals. |

## Architecture

```text
 Windows / Linux / macOS
          │
          │ local usage files / databases
          ▼
     Tokscale Core v4.14.0
  parsing · dedup · token semantics
  model normalization · pricing
          │
          ▼
      token-monitor
  route evidence + encryption
          │
          │ AES-256-GCM snapshot
          ▼
 YOUR_NAME/token-monitor fork
 ├─ main                    project source
 ├─ tm-ledger-<device-A>    encrypted ledger.json
 ├─ tm-ledger-<device-B>    encrypted ledger.json
 └─ tm-ledger-<device-C>    encrypted ledger.json
          │
          ▼
 Atingaii GitHub Pages dashboard
 browser-local decrypt + analytics
```

A device sync never writes telemetry to `main`. Each device controls only its own `tm-ledger-<device-hash>` branch, so devices do not contend on a shared file. Snapshot branches are force-moved to fresh root commits instead of growing a visible telemetry commit history.

## Features

### Multi-client accounting

Token Monitor scans the client set exposed by the pinned Tokscale Core rather than implementing separate parsers itself. This includes Codex, Claude Code, OpenCode, Gemini-related sources, Kimi, Cursor-related sources, DeepSeek Harness, Copilot sources, and the broader client set supported by Tokscale v4.14.0.

Run this on any installed machine to see the exact embedded set:

```bash
token-monitor clients
```

See [`SOURCES.md`](SOURCES.md) for the accounting provenance and version pins.

### Provider and route identity

The ledger keeps four identities separate:

| Field | Meaning | Example |
| --- | --- | --- |
| `model` | Canonical model | `gpt-5.6-sol` |
| `upstreamVendor` | Model family owner | `openai` |
| `routeProvider` | Actual route/billing provider when evidenced | `azure-openai`, `aws-bedrock`, `openrouter`, `newapi` |
| `routeType` | Route class | `official`, `cloud`, `aggregator`, `relay`, `self-hosted`, `unknown` |

This intentionally prevents a common analytics error: **model identity is not route identity**. If the source proves only that the model belongs to OpenAI, the upstream vendor may be `openai` while the route remains `unknown`.

Recognized route classes include official APIs, Azure OpenAI, AWS Bedrock, Google Vertex, OpenRouter, New API / One API / LiteLLM / CLIProxyAPI-style relays, inference providers, self-hosted endpoints, and unknown/custom routes when evidence is incomplete.

### Codex service tiers

Codex request-level Fast/Standard handling is a narrow enhancement derived from the MIT-licensed `falyx6851-byte/codex-monitor` service-tier parser and pricing logic. It is **not** allowed to redefine canonical Codex usage.

For a day to receive Fast/Standard detail, the enhancement must reconcile with Tokscale on both token total and message count. If it does not, Token Monitor discards the tier split for that day and keeps the canonical Tokscale accounting intact.

### API-equivalent cost

`costUsd` means **API-equivalent estimated value**, not a ChatGPT/Codex subscription invoice.

Generic model pricing follows Tokscale. Specialized Codex service-tier pricing is isolated to the tier adapter. Where a source cannot expose a billable component such as cache-write tokens, the affected row is marked as a **lower-bound estimate** instead of being presented as exact.

## Dashboard

The static dashboard is designed as a restrained admin/analytics console rather than a card-heavy decorative UI.

It supports:

- Today / 7 days / 30 days / current month / all time / custom range
- device, client, model, upstream vendor, route provider, route type, raw provider, and service-tier filters
- Total Tokens, Input, Cache Read, Cache Write, Output, Reasoning, Messages, and API-equivalent Cost
- line, area, bar, stacked bar, donut, treemap, and table views
- CSV export
- light and dark themes
- browser-local AES-GCM decryption

A user's fork does **not** need its own GitHub Pages deployment. The central dashboard reads the encrypted `tm-ledger-*` branches from the repository encoded in its URL.

## Quick Start

### 1. Install

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

**Windows PowerShell**

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

Installers download the matching prebuilt release binary, verify its SHA-256 checksum, and do not install Rust, Node.js, or Python.

### 2. First device

```bash
token-monitor setup
```

`setup` automatically:

1. resolves a GitHub write credential from explicit flags, environment variables, or an authenticated `gh` session;
2. finds your `token-monitor` fork, or attempts to fork the project automatically;
3. generates a dashboard encryption key;
4. performs the first full usage scan;
5. writes the first encrypted device snapshot;
6. installs the low-overhead native scheduler;
7. prints your Dashboard URL and the exact join command for another device.

No extra `token-monitor-data` repository is required.

If your fork was renamed or is organization-owned, use the advanced override:

```bash
token-monitor setup --repo OWNER/RENAMED_FORK
```

### 3. Add another device

The first machine prints a command like:

```bash
token-monitor join 'eyJ2ZXJzaW9uIjoyLC4uLn0'
```

Paste that command unchanged on another Windows, Linux, or macOS machine. To print it again later:

```bash
token-monitor invite
```

The pair code contains the fork address, dashboard decryption key, and sync cadence. It does **not** contain a GitHub write token.

## Authentication

Token Monitor resolves a GitHub credential in this order:

1. explicit `--token`
2. `TOKEN_MONITOR_GITHUB_TOKEN`
3. `GITHUB_TOKEN`
4. `GH_TOKEN`
5. authenticated `gh auth token`
6. one-time hidden terminal prompt

The credential is used only to manage the user's fork and encrypted snapshot branches. It is never embedded in the pair code or dashboard URL.

## Background load

There is no resident Token Monitor process.

| Platform | Scheduler |
| --- | --- |
| Windows | Task Scheduler |
| macOS | `launchd` |
| Linux | `systemd --user` timer, with cron fallback |

The default schedule is one incremental sync every 15 minutes. Incremental scans rescan a two-day overlap to account for delayed writes and day boundaries. If accounting has not changed, the client skips the GitHub snapshot write entirely.

## Privacy model

Uploaded ledgers contain normalized aggregate statistics such as:

- date and device identity
- client, model, provider/route labels, optional service tier
- input/cache/output/reasoning token buckets
- message count
- API-equivalent cost and lower-bound flag where applicable

They do **not** upload:

- prompts or assistant responses
- reasoning text
- source code or project contents
- project paths
- full JSONL/SQLite session data
- `auth.json`
- API keys or GitHub tokens

Before upload, the ledger is encrypted using AES-256-GCM with a random 256-bit dashboard key. The dashboard key is carried in the URL fragment (`#key=...`), which browsers do not send as part of the HTTP request.

## Commands

```text
token-monitor setup [--repo OWNER/REPO]
token-monitor join <PAIR_CODE>
token-monitor invite
token-monitor sync [--full]
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor uninstall [--remove-remote] [--purge]
```

## Verification

The release gate is intentionally broader than “it compiles on Linux”. CI covers native or hosted runners for:

- Linux x86_64
- Linux ARM64
- Windows x86_64
- Windows ARM64
- macOS Intel
- macOS Apple Silicon
- Rust `clippy`
- dashboard syntax, aggregation, CSV, and privacy regressions
- the complete pinned Tokscale v4.14.0 parser/scanner regression suite

The release workflow publishes platform-specific binaries for the same six OS/architecture targets.

## Build from source

End users do not need Rust. Contributors can build the workspace with:

```bash
cargo test --workspace --all-targets
cargo build --release --workspace
```

## Project structure

```text
rust-cli/     lightweight collector, encryption, GitHub sync and scheduling
web/          static analytics dashboard
.github/      CI, release, Pages workflows and project artwork
SOURCES.md    accounting provenance and pinned upstream implementations
```

## Contributing

Bug reports and pull requests are welcome. Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) before changing accounting, provider attribution, crypto, or release behavior. Accounting changes must retain source provenance and test coverage rather than introducing a second hand-written parser or pricing implementation.

## Security

For credential handling, encrypted ledger behavior, or other security-sensitive issues, see [`SECURITY.md`](SECURITY.md). Please do not publish sensitive reproduction data in a public issue.

## Upstream and attribution

This project is a serverless CLI-focused rewrite of [Javis603/token-monitor](https://github.com/Javis603/token-monitor) and uses [junhoyeo/tokscale](https://github.com/junhoyeo/tokscale) v4.14.0 as its multi-client accounting engine. Codex service-tier enhancement is derived from the MIT-licensed [falyx6851-byte/codex-monitor](https://github.com/falyx6851-byte/codex-monitor).

Detailed provenance is recorded in [`SOURCES.md`](SOURCES.md) and [`NOTICE`](NOTICE).

## License

[MIT](LICENSE). Original upstream notices are retained as required.
