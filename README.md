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

<p align="center"><strong>One lightweight CLI · Every device · Multi-client accounting · One zero-login dashboard</strong></p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="https://atingaii.github.io/token-monitor/">Live Dashboard</a> ·
  <a href="SOURCES.md">Accounting Sources</a>
</p>

Token Monitor is a **serverless, cross-device analytics console for AI coding tools**. A prebuilt Rust CLI scans local usage with the pinned Tokscale Core accounting engine, writes one history-free snapshot branch per device into your own public Token Monitor fork, and exposes strictly de-identified aggregate usage through a static GitHub Pages dashboard.

No Electron, Node.js, Python, Docker, VPS, database, always-on hub, or second data repository is required.

> **Privacy model:** the dashboard is intentionally zero-login and public. `public.json` contains only de-identified aggregate accounting rows. Prompts, responses, reasoning text, source code, project paths, session IDs, credentials, hostnames, configured device names and raw device IDs are excluded. See [Security](#privacy--security) before using a public dashboard.

## Why Token Monitor

Most monitors answer *"what did this machine use?"*. Token Monitor answers cross-device questions:

- How many tokens did all machines use together?
- Which client, model, route provider, route type, service tier or anonymous device consumed them?
- Was a GPT/Claude model routed through an official endpoint, a cloud provider, OpenRouter or a relay?
- What is the **current API-equivalent** value of the observed usage?
- For reconciled Codex tiers, what is a separate **included-plan / legacy-meter equivalent** planning value?
- Can this be done without a permanent monitoring process or server?

## Core principles

| Principle | Implementation |
| --- | --- |
| **Low overhead** | Native schedulers launch a short one-shot sync and exit; idle resident memory is zero. |
| **Cross platform** | Native CI/release targets for Windows, Linux and macOS on x64 + ARM64. |
| **Mature accounting core** | Parsing, dedup, token semantics, model normalization and general API pricing come from pinned **Tokscale v4.14.0**. |
| **Evidence-first routing** | Model vendor and actual request route are different dimensions; model name alone never proves `official`. |
| **Exact Codex reconciliation** | Fast/Standard subdivision is accepted only when token + message totals match canonical Tokscale for that day. |
| **Zero-login dashboard** | Any browser can open the public aggregate dashboard; no per-browser key is required. |
| **Explicit public boundary** | `public.json` is de-identified aggregate data; an encrypted `ledger.json` is retained only for compatibility. |
| **No telemetry history** | Every `tm-ledger-*` branch is force-moved to a fresh root snapshot commit. |

## Architecture

```text
 Windows / Linux / macOS
          │
          │ local AI coding usage files / databases
          ▼
     Tokscale Core v4.14.0
 parsing · dedup · token semantics
 model normalization · API pricing
          │
          ▼
      token-monitor
 route evidence · Codex tier adapter
 public sanitization · compatibility encryption
          │
          ▼
 YOUR_NAME/token-monitor fork
 ├─ main                    project source
 ├─ tm-ledger-<device-A>
 │   ├─ public.json         de-identified aggregate
 │   └─ ledger.json         AES-GCM compatibility aggregate
 ├─ tm-ledger-<device-B>
 └─ ...
          │
          ▼
 https://atingaii.github.io/token-monitor/
 public, static, zero-login analytics
```

Telemetry never lands on `main`. Each device owns one branch, so multiple machines do not contend on a shared file.

## Dashboard

The v1.0.1 dashboard is modeled after the restrained layout language of modern New API admin consoles: compact typography, white/gray surfaces, subtle borders, restrained blue accents and dense but readable information hierarchy.

Highlights:

- collapsible desktop sidebar with persisted expanded/collapsed state;
- responsive desktop/tablet/mobile layout;
- light and dark themes;
- Today / 7d / 30d / current month / all / custom ranges;
- device, client, model, upstream vendor, route provider, route type, raw provider and Tier filters;
- Total Tokens, Input, Cache Read, Cache Write, Output, Reasoning and Messages;
- separate **Plan-equivalent** and **Current API-equivalent** cost metrics;
- line, area, bar, stacked bar, donut, treemap and table views;
- CSV export;
- anonymous device labels instead of local host/device names.

Open the central dashboard directly:

**https://atingaii.github.io/token-monitor/**

For another user's fork, use `?repo=OWNER/token-monitor`.

## Two cost estimates

A single dollar number was misleading because current GPT-5.6 API pricing and included-plan / legacy-meter behavior are not the same basis.

### Plan-equivalent cost

`planCostUsd` is a **planning equivalent**, not an invoice. It is only populated when the Codex service tier can be reconstructed and reconciled with Tokscale.

For GPT-5.6 Sol, Token Monitor preserves the original Sol launch basis for this estimate because OpenAI states that the later promotional reduction does not change included plan usage, 5-hour/weekly limits or legacy credit rates. Fast-mode planning uses the applicable Codex/Work meter multiplier on that plan basis.

### Current API-equivalent cost

`costUsd` represents current API-style value. General model pricing comes directly from Tokscale `PricingService`; reconciled GPT-5.6 Codex requests use the current official Standard/Fast API tables.

Neither value is a subscription bill. Unknown or incomplete pricing evidence is not guessed; affected values are marked as lower bounds.

See [`SOURCES.md`](SOURCES.md) for the exact implementation and official source mapping.

## Multi-client accounting

Token Monitor does not maintain dozens of custom parsers. It scans the client set exposed by pinned Tokscale Core, including Codex, Claude Code, OpenCode, Gemini-family sources, Kimi, Cursor-family sources, Copilot sources and the broader Tokscale v4.14.0 client set.

```bash
token-monitor clients
```

## Provider and route identity

The ledger deliberately separates:

| Dimension | Meaning | Example |
| --- | --- | --- |
| `model` | canonical model | `gpt-5.6-sol` |
| `upstreamVendor` | model family owner | `openai` |
| `routeProvider` | evidenced route/billing provider | `azure-openai`, `aws-bedrock`, `openrouter`, `newapi` |
| `routeType` | route class | `official`, `cloud`, `aggregator`, `relay`, `self-hosted`, `unknown` |
| `provider` | raw source provider label | source-specific |
| `tier` | optional service tier | `standard`, `fast` |

A GPT model may have `upstreamVendor=openai` while `routeProvider=unknown`; model identity alone never proves the official route.

## Quick Start

### Install

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

**Windows PowerShell**

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

The installer downloads the matching prebuilt binary and verifies its SHA-256 checksum.

### First device

```bash
token-monitor setup
```

`setup` automatically discovers/creates your project fork, performs a full scan, publishes the first history-free snapshot, installs the native low-overhead scheduler, prints the public dashboard URL and prints a copy-paste `join` command for another device.

Advanced renamed/org fork:

```bash
token-monitor setup --repo OWNER/RENAMED_FORK
```

### Additional device

Paste the command printed by the first machine:

```bash
token-monitor join '<PAIR_CODE>'
```

Print it again later with:

```bash
token-monitor invite
```

The pair code does **not** contain a GitHub credential. Compatibility encryption material remains in the pair code so joined devices can maintain the encrypted compatibility ledger, but the public dashboard itself does not require that key.

## Upgrade from v1.0.0

v1.0.1 changes the public dashboard format. Existing v1.0.0 branches contain only encrypted `ledger.json`, so upgrade each existing machine once and force a full sync:

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
token-monitor sync --full
token-monitor dashboard
```

Ledger schema v4 deliberately forces the migration write. After each device has synced with v1.0.1+, `public.json` exists and any browser can open the dashboard without a key.

## Background load

There is no resident Token Monitor process.

| Platform | Scheduler |
| --- | --- |
| Windows | Task Scheduler |
| macOS | `launchd` |
| Linux | `systemd --user` timer, cron fallback |

Default cadence: every 15 minutes. Incremental scans use a two-day overlap. If accounting is unchanged, no GitHub write occurs.

## Privacy & security

### Public `public.json`

Intentionally public aggregate fields include date, anonymous device hash/label, platform/arch, client/model/route/tier labels, additive token buckets, message count, cost estimates and snapshot timing metadata.

It excludes:

- configured device name and hostname;
- raw/original device ID;
- session IDs and full session records;
- prompts and assistant responses;
- reasoning text;
- source code and project contents;
- project/workspace paths;
- `auth.json` and auth files;
- API keys, GitHub tokens, cookies, passwords and pair codes.

A regression test asserts that local identity fields cannot pass through `PublicLedger::from_ledger`.

### Encrypted `ledger.json`

Each branch also retains an AES-256-GCM encrypted compatibility aggregate. It is no longer required by the normal dashboard path.

A JavaScript-only password on a public static site would not meaningfully protect public data; if your aggregate usage itself must remain private, this public-dashboard mode is not the correct deployment model. See [`SECURITY.md`](SECURITY.md).

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

The release gate covers:

- Linux x86_64 / ARM64;
- Windows x86_64 / ARM64;
- macOS Intel / Apple Silicon;
- Rust `clippy`;
- dashboard JS syntax, analytics and privacy regressions;
- complete pinned Tokscale v4.14.0 parser/scanner regression suite.

## Build from source

```bash
cargo test --workspace --all-targets
cargo build --release --workspace
```

## Project structure

```text
rust-cli/     collector, tier adapter, sanitization, GitHub sync, scheduling
web/          static zero-login analytics dashboard
.github/      CI, release, Pages workflows and project artwork
SOURCES.md    accounting and pricing provenance
SECURITY.md   exact public/private data boundary
```

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before changing accounting, pricing, provider attribution, public schemas, crypto or release behavior. Accounting changes should extend established upstream implementations rather than introduce duplicate parsers.

## Upstream & attribution

Token Monitor is a serverless CLI-focused rewrite of [Javis603/token-monitor](https://github.com/Javis603/token-monitor), uses [junhoyeo/tokscale](https://github.com/junhoyeo/tokscale) v4.14.0 as the multi-client accounting engine, and derives the narrow Codex service-tier adapter from MIT-licensed [falyx6851-byte/codex-monitor](https://github.com/falyx6851-byte/codex-monitor).

See [`SOURCES.md`](SOURCES.md) and [`NOTICE`](NOTICE).

## License

[MIT](LICENSE).
