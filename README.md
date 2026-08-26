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
  <strong>One lightweight CLI. Every device. Mature token accounting. One encrypted analytics dashboard.</strong>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#dashboard-access">Dashboard Access</a> ·
  <a href="#pricing-policy">Pricing</a> ·
  <a href="SOURCES.md">Sources</a>
</p>

Token Monitor is a **serverless, cross-device analytics system for AI coding tools**. A prebuilt Rust CLI scans local usage through pinned Tokscale Core, keeps model vendor and actual request route separate, calculates a mature-source subscription-equivalent cost, encrypts each device snapshot, and synchronizes it to the user's own Token Monitor fork.

The central GitHub Pages dashboard is a static application. It reads encrypted snapshot branches and decrypts them locally after the user enters one memorable dashboard password. There is no Electron app, Node.js runtime, Python environment, Docker stack, VPS, database, permanent hub process, or second telemetry repository to operate.

> **v1.1 changes the dashboard and pricing model.** The dashboard no longer requires a long `#key=...` URL. GPT-5.6 pricing follows the CC Switch-compatible subscription-equivalent rate policy rather than the lower generic API catalog price.

## Why Token Monitor

Token Monitor is built for questions a single-machine counter cannot answer:

- How many tokens did all of my machines use together?
- Which device, client, model, provider route, or service tier consumed them?
- Was a GPT/Claude request routed through an official API, a cloud provider, OpenRouter, or a relay?
- What is the **subscription-equivalent** value of that usage under a documented mature-project rate policy?
- Can this run continuously without leaving a monitoring daemon in memory?

## Core principles

| Principle | Implementation |
| --- | --- |
| **Low overhead** | No resident daemon. Native OS schedulers launch a short one-shot sync and exit. |
| **Cross platform** | Native release/test targets for Windows, Linux, and macOS on x64 and ARM64. |
| **Mature token accounting** | Client discovery, parsing, deduplication and token-bucket semantics come from pinned **Tokscale v4.14.0**. |
| **Mature pricing sources** | General prices follow CC Switch's models.dev source; GPT-5.6 uses guarded CC Switch rates independently cross-checked against Sub2API. |
| **Evidence-first routing** | Model vendor and actual request route are separate. A GPT model alone never proves “OpenAI official”. |
| **Encrypted telemetry** | Aggregate device ledgers remain AES-256-GCM encrypted in `tm-ledger-*` branches. |
| **Memorable dashboard access** | A user password wraps the random workspace key with PBKDF2 + AES-GCM; the password is not stored or put in the URL. |
| **Fail closed on tier attribution** | Codex tier rows can replace grouping only when their daily additive token total exactly reconciles with Tokscale. |

## Architecture

```text
 Windows / Linux / macOS
          │
          │ local usage files / databases
          ▼
      Tokscale Core v4.14.0
  parsing · dedup · token semantics
        model normalization
          │
          ├──────────────► Codex tier evidence adapter
          │                 (request-level only)
          ▼
      token-monitor
  route evidence · pricing · encryption
          │
          ├─ AES-256-GCM device ledger
          │
          └─ password-wrapped workspace key
                     │
                     ▼
 YOUR_NAME/token-monitor fork
 ├─ main                       project source
 ├─ tm-dashboard               access.json only
 ├─ tm-ledger-<device-A>       encrypted ledger.json
 ├─ tm-ledger-<device-B>       encrypted ledger.json
 └─ tm-ledger-<device-C>       encrypted ledger.json
                     │
                     ▼
 https://atingaii.github.io/token-monitor/
 password → local key unwrap → local ledger decrypt
```

Device sync never writes telemetry to `main`. Each device controls only its own history-free `tm-ledger-<device-hash>` root snapshot. `tm-dashboard` contains only the password-wrapped random workspace key; it does not contain the password or raw usage.

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

The installers download the native release asset and its `.sha256` file, verify the checksum, and run a native `--version` smoke check. Rust, Node.js, Python and Docker are not required.

> **Linux PATH note:** a `curl | sh` child shell cannot mutate the PATH of its parent shell. The installer therefore prefers an already-active user PATH directory. If none exists, it prints the exact absolute `.../token-monitor setup` command that works immediately; new terminals receive the profile PATH update.

### 2. First device

```bash
token-monitor setup
```

Setup automatically:

1. resolves a GitHub write credential from a flag, environment variable, authenticated `gh`, or a hidden prompt;
2. finds your Token Monitor fork or creates it when possible;
3. creates the random 256-bit workspace encryption key;
4. asks you to create a **dashboard password** in a hidden prompt;
5. publishes only a password-wrapped copy of the random workspace key to `tm-dashboard/access.json`;
6. performs the first full local scan and encrypted snapshot;
7. installs the native low-overhead scheduler;
8. prints the stable Dashboard URL and the join command for another device.

The URL is short and stable:

```text
https://atingaii.github.io/token-monitor/?repo=YOUR_NAME/token-monitor
```

There is no `#key=...` suffix in v1.1.

For non-interactive setup, use `TOKEN_MONITOR_DASHBOARD_PASSWORD`. Avoid putting a real password directly in shell history with `--dashboard-password` unless you understand the exposure.

### 3. Open the dashboard

Open the same URL in **any browser** and enter the dashboard password. Nothing needs to have been stored in that browser beforehand.

### 4. Add another device

The first machine prints a command like:

```bash
token-monitor join 'eyJ2ZXJzaW9uIjoyLC4uLn0'
```

Paste it unchanged on another Windows, Linux, or macOS machine. To print it again:

```bash
token-monitor invite
```

The pair code contains the repository, random workspace key, and sync cadence. It does **not** contain a GitHub credential. Treat a pair code as a workspace secret because a joined device must be able to encrypt/decrypt the shared ledgers.

## Upgrading from v1.0

Re-run the installer to replace the binary, then on **one existing configured device** run:

```bash
token-monitor password
token-monitor sync --full
```

The first command creates `tm-dashboard/access.json` from your existing workspace key without changing that key. The second rewrites the local ledger with the v1.1 pricing metadata and current pricing policy. Existing v1.0 fragment URLs remain readable as a migration path, but new `token-monitor dashboard` output never exposes the workspace key in the URL.

## Dashboard access

The dashboard password is **not** the AES ledger key.

1. Setup generates a random 256-bit workspace key.
2. Your password is processed with **PBKDF2-HMAC-SHA256 (310,000 iterations)** and a random salt.
3. The derived key AES-256-GCM encrypts the random workspace key.
4. Only that small envelope is stored as `tm-dashboard/access.json`.
5. A browser repeats the PBKDF2 derivation and unwraps the workspace key locally.
6. Device ledgers are then decrypted locally with the random workspace key.

The password is not uploaded to GitHub, embedded in the pair code, or appended to the Dashboard URL.

Because this is still a public static-site architecture, an attacker can download the password-wrapped envelope and attempt offline guesses. Use a strong, memorable passphrase rather than a trivial 8-character password. This is not a server-side account/login system.

Change the dashboard password from any configured device:

```bash
token-monitor password
```

Changing the password re-wraps the same workspace key; it does not rewrite all device ledgers.

## Pricing policy

The Dashboard reports **subscription-equivalent estimated cost**, not an OpenAI/Anthropic invoice and not the number of dollars deducted from a ChatGPT/Codex subscription.

### General models

The rate catalog follows the mature CC Switch integration with:

```text
https://models.dev/api.json
```

Token Monitor applies those prices only after Tokscale has already normalized raw counters into fresh input, cache read, cache write, output, and reasoning buckets. Unknown prices are not inferred from nearby model names.

### GPT-5.6 guarded rates

GPT-5.6 intentionally does **not** follow a lower generic API catalog rate when that would change the intended subscription-equivalent accounting policy. The guarded card follows CC Switch and is independently cross-checked against Sub2API:

| Model | Input / MTok | Output / MTok | Cache read / MTok | Cache write / MTok |
| --- | ---: | ---: | ---: | ---: |
| `gpt-5.6-sol` | **$5.00** | **$30.00** | **$0.50** | **$6.25** |
| `gpt-5.6-terra` | $2.00 | $12.00 | $0.20 | $2.50 |
| `gpt-5.6-luna` | $0.20 | $1.20 | $0.02 | $0.25 |

`gpt-5.6` and its effort aliases are treated as Sol. GPT-5.6 `fast` / `priority` uses the explicit **2×** price card.

For a request with more than **272K total input-side tokens**, the compatible long-context policy is applied at **request granularity before aggregation**:

- fresh input × 2
- cache read × 2
- cache write × 2
- output (including reasoning tokens) × 1.5

If a raw client does not expose a billable bucket separately—for example a missing cache-write field—the row is marked as a **lower-bound** cost rather than presented as exact.

Full provenance is in [`SOURCES.md`](SOURCES.md).

## Codex service tiers

Tokscale remains the canonical Codex token total. A narrow adapter derived from the MIT-licensed `falyx6851-byte/codex-monitor` state machine extracts request-level `standard` / `fast` evidence.

The tier split is accepted only when its **daily additive token total exactly equals Tokscale's daily total**. The two parsers' record/message counts are allowed to differ because they expose different record granularities. This fixes the v1.0 behavior where valid Fast evidence could be discarded merely because record counts did not match.

Tier requests remain request-granular through pricing so the 272K threshold cannot accidentally be applied to a whole day's aggregate.

## Provider and route identity

The ledger keeps distinct fields for model and route identity:

| Field | Meaning | Example |
| --- | --- | --- |
| `model` | Canonical model | `gpt-5.6-sol` |
| `upstreamVendor` | Model family owner | `openai` |
| `routeProvider` | Actual route/billing provider when evidenced | `azure-openai`, `aws-bedrock`, `openrouter`, `newapi` |
| `routeType` | Route class | `official`, `cloud`, `aggregator`, `relay`, `self-hosted`, `unknown` |

A model-family inference may set `upstreamVendor`; it never proves `routeType=official`. Official/cloud/relay identity requires source or configuration evidence.

## Dashboard

The v1.1 dashboard uses a restrained modern admin-console layout:

- collapsible desktop sidebar (collapse state is only a UI preference)
- responsive mobile drawer
- Today / 7d / 30d / current month / all / custom ranges
- device, client, model, upstream vendor, route provider, route type, raw provider and tier filters
- Total Tokens, subscription-equivalent cost, input, cache, output/reasoning and record KPIs
- line, area, bar, stacked bar, donut, treemap and table views
- device status table and raw aggregate-data table
- CSV export
- light/dark themes
- visible pricing-source provenance
- browser-local PBKDF2 + AES-GCM unlock/decryption

A user's fork does **not** need its own GitHub Pages deployment. The central dashboard reads encrypted branches from the repository in `?repo=OWNER/REPO`.

## Multi-client accounting

Token Monitor scans the client set exposed by pinned Tokscale Core instead of maintaining independent parsers. Run:

```bash
token-monitor clients
```

for the exact embedded client list on the current release.

## GitHub authentication

Credential resolution order:

1. `--token`
2. `TOKEN_MONITOR_GITHUB_TOKEN`
3. `GITHUB_TOKEN`
4. `GH_TOKEN`
5. authenticated `gh auth token`
6. hidden terminal prompt

The credential is stored only in the device's private local configuration and is used to manage encrypted snapshot/access branches in the selected fork. It is never sent to the static dashboard.

## Background load

There is no resident Token Monitor process.

| Platform | Scheduler |
| --- | --- |
| Windows | Task Scheduler |
| macOS | `launchd` |
| Linux | `systemd --user`, with cron fallback |

The default cadence is 15 minutes. Incremental scans rescan a two-day overlap to accommodate delayed writes/day boundaries. When accounting and pricing identity have not changed, no new GitHub snapshot is written.

## Windows and Linux notes

### Linux

If the installer reports that its install directory was not already on your current PATH, use the exact absolute setup command printed by the installer. This is necessary because POSIX does not allow a child `curl | sh` process to mutate its parent's environment.

The scheduler first tries a user `systemd` timer. Headless/minimal systems without a usable user bus fall back to `crontab`. If neither is available, setup remains usable and tells you to run `token-monitor sync` manually.

### Windows

The installer supports Windows PowerShell 5.1 and modern PowerShell, enables TLS 1.2 compatibility where necessary, installs under Local App Data by default, updates both the current process PATH and user PATH, and prints the absolute executable path as a fallback.

Task Scheduler receives a quoted native executable path so usernames/install paths containing spaces remain supported.

## Privacy model

Encrypted ledgers contain only normalized aggregate fields such as:

- date and device identity
- client, model, provider/route labels, optional service tier
- input/cache/output/reasoning token buckets
- additive record count
- subscription-equivalent cost and lower-bound flag
- pricing provenance metadata

They do **not** contain:

- prompts or assistant responses
- reasoning text
- source code or project contents
- project paths
- full JSONL/SQLite session data
- `auth.json`
- API keys or GitHub tokens

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

## Verification and release gate

Every pull request is tested on native/hosted runners for:

- Linux x86_64
- Linux ARM64
- Windows x86_64
- Windows ARM64
- macOS Apple Silicon
- macOS Intel
- Rust `clippy`
- dashboard JavaScript/analytics/privacy regressions
- the complete pinned `tokscale-core` v4.14.0 parser/scanner suite

Release builds repeat native tests/builds for all six targets. **After the GitHub Release is published**, a second six-runner matrix downloads the public release through the real `install.sh` / `install.ps1`, verifies SHA-256, executes the installed binary and runs CLI smoke tests. A successful build artifact alone is therefore not the final release gate.

## Development

```bash
git clone https://github.com/Atingaii/token-monitor.git
cd token-monitor
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets
node web/analytics.test.cjs
```

## Sources and license

Token Monitor is MIT licensed. Significant implementation provenance is documented in [`NOTICE`](NOTICE) and [`SOURCES.md`](SOURCES.md).

Primary mature sources include:

- [`junhoyeo/tokscale`](https://github.com/junhoyeo/tokscale) v4.14.0 — client parsing/token accounting (MIT)
- [`falyx6851-byte/codex-monitor`](https://github.com/falyx6851-byte/codex-monitor) — Codex request/tier evidence behavior (MIT)
- [`farion1231/cc-switch`](https://github.com/farion1231/cc-switch) — pricing behavior and models.dev integration (MIT)
- [`Wei-Shaw/sub2api`](https://github.com/Wei-Shaw/sub2api) — independent GPT-5.6 rate/policy cross-check only (LGPL-3.0; no source code copied or linked)

For security issues, see [`SECURITY.md`](SECURITY.md). For contribution guidance, see [`CONTRIBUTING.md`](CONTRIBUTING.md).
