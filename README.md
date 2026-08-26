<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor" width="100%" />
</p>

<p align="center">
  <img alt="v1.1.0" src="https://img.shields.io/badge/version-v1.1.0-2563eb?style=flat-square">
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-111827?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-52525b?style=flat-square">
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-16a34a?style=flat-square"></a>
</p>

<p align="center"><strong>One lightweight dashboard for token usage, routing, and subscription-equivalent cost across AI coding clients and devices.</strong></p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="https://token-monitor-cuidongshan350-1312.vercel.app/">Dashboard</a> ·
  <a href="SOURCES.md">Sources</a> ·
  <a href="SECURITY.md">Security</a>
</p>

## What is Token Monitor?

Token Monitor is a **serverless, cross-device, CLI-only** usage console for AI coding tools. It reads local usage records, delegates mature client parsing and token semantics to Tokscale Core, adds route evidence and subscription-equivalent pricing, encrypts each device snapshot, and renders everything in one static dashboard.

There is no Electron app, Node daemon, VPS, database, or separate telemetry repository. Each sync is a short-lived process that exits when it is done.

## v1.1 highlights

| Capability | Implementation |
| --- | --- |
| Client parsing | Tokscale Core v4.14.0 for discovery, parsing, dedup, token semantics and model normalization |
| Platforms | Windows, Linux and macOS; x64 and ARM64 |
| Dashboard access | One stable URL and one memorable password from any browser |
| Encryption | AES-256-GCM; PBKDF2-HMAC-SHA256 wraps the random workspace key |
| Scheduling | launchd / Task Scheduler / systemd-user / cron; no resident daemon |
| Route analytics | upstream vendor, route provider, route type and raw provider kept separate |
| Official routes | Proven first-party routes render as **Official** while raw provider evidence remains auditable |
| Pricing | General catalog follows the same models.dev source used by CC Switch; GPT-5.6 is cross-checked against CC Switch/Sub2API |
| UI | Restrained New API-inspired admin layout, collapsible sidebar, precise charts, CSV export |
| Linux | Static musl v1.1 artifacts avoid old-glibc `GLIBC_2.xx not found` failures |
| Windows | PowerShell 5.1 / 7 compatible installer with x64 / ARM64 detection |

## Install

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

The v1.1 Linux build is statically linked with musl, so it does not require a glibc as new as the build runner.

If a `curl | sh` child shell cannot update the current parent shell's PATH, the installer prints an immediately usable absolute command.

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

Do **not** use the Unix `curl -fsSL ... | sh` form in Windows PowerShell: `curl` is commonly an alias for `Invoke-WebRequest` there.

The installer supports Windows PowerShell 5.1 and PowerShell 7, validates SHA-256, detects native x64/ARM64, and handles user PATH updates.

## First device

```bash
token-monitor setup
```

Setup automatically:

1. resolves GitHub credentials;
2. finds or creates your Token Monitor fork;
3. creates a random 256-bit workspace key;
4. asks for a memorable dashboard password;
5. publishes only the password-wrapped access manifest;
6. performs a full local scan;
7. uploads the device's encrypted snapshot;
8. installs the native low-load scheduler;
9. prints the dashboard and join command.

Primary dashboard:

**https://token-monitor-cuidongshan350-1312.vercel.app/**

No `#key=...` is required. A different browser, computer, phone, or private window only needs the same password.

## Add another device

The first device prints:

```bash
token-monitor join '<PAIR_CODE>'
```

Each machine uses its own GitHub write credential. The pair code contains the repository, workspace key, and cadence; it does not contain a GitHub token.

Print it again with:

```bash
token-monitor invite
```

## Upgrade from v1.0

Re-run the installer, then on an existing configured device:

```bash
token-monitor password
token-monitor sync --full
```

v1.1 uses a newer ledger/pricing schema. Old cached pricing is not mixed with v1.1 pricing; the migration rebuilds historical accounting once and then returns to incremental scans.

## Pricing semantics

The dashboard shows **subscription-equivalent cost**, not a provider invoice and not the amount charged by a ChatGPT/Codex subscription.

### GPT-5.6

USD per 1M tokens:

| Model | Input | Cache read | Cache write | Output |
| --- | ---: | ---: | ---: | ---: |
| GPT-5.6 Sol | **$5.00** | **$0.50** | **$6.25** | **$30.00** |
| GPT-5.6 Terra | $2.00 | $0.20 | $2.50 | $12.00 |
| GPT-5.6 Luna | $0.20 | $0.02 | $0.25 | $1.20 |

Fast / Priority uses the explicit **2×** tier card. The `>272K` long-context rule is evaluated per request before aggregation.

Example:

```text
182,000 fresh input × $5/M
6,080,000 cache read × $0.50/M
12,000 output × $30/M
≈ $4.31
```

That is why the old API-style `$4/$20/$0.40` interpretation lands around `$3.40`, while the v1.1 subscription-equivalent interpretation lands around `$4.31`.

General model pricing follows the same `models.dev` catalog used by CC Switch. Unknown or incomplete price coverage is marked as a lower bound instead of being guessed from a neighboring model.

See [`SOURCES.md`](SOURCES.md) for provenance and licenses.

## Route semantics

A GPT or Claude model name alone does **not** prove an official route.

| Field | Meaning | Example |
| --- | --- | --- |
| `model` | canonical model | `gpt-5.6-sol` |
| `upstreamVendor` | model family/vendor | `openai` |
| `routeProvider` | actual route | `official`, `azure-openai`, `openrouter`, `newapi` |
| `routeType` | route class | `official`, `cloud`, `aggregator`, `relay`, `unknown` |
| `provider` | raw source evidence | retained for auditing |

Once first-party evidence is sufficient, the UI renders the route as **Official**. Non-official routes keep their actual provider name.

## Dashboard

v1.1 uses a restrained modern admin-dashboard language inspired by the information density and layout direction of current New API, with an independent implementation.

It includes:

- collapsible desktop sidebar and mobile drawer;
- light/dark modes;
- today / 7d / 30d / month / all / custom ranges;
- device, client, model, vendor, route, provider and tier filters;
- line, area, bar, stacked bar, donut, treemap and table views;
- exact hover values and comma-separated table values;
- adaptive Y-axis nice scaling with K/M/B/T labels;
- CSV export.

## Privacy

Remote ledgers contain aggregate counters and the minimum routing labels required for analytics. Token Monitor does **not** upload:

- prompts or assistant replies;
- reasoning text;
- source code or project contents;
- project paths;
- full transcripts;
- `auth.json`;
- API keys;
- GitHub tokens.

Each device owns a `tm-ledger-*` snapshot branch. `main` remains source code only.

## Commands

```bash
token-monitor setup
token-monitor sync
token-monitor sync --full
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor invite
token-monitor password
token-monitor uninstall
```

## Low-load design

```text
native scheduler
   ↓ every ~15 minutes
start token-monitor sync --quiet
   ↓
incremental local scan
   ↓
changed → replace encrypted snapshot
unchanged → no GitHub write
   ↓
process exits
```

Resident memory between scheduled syncs: **0**.

## Provenance

Core parsing comes from [`junhoyeo/tokscale`](https://github.com/junhoyeo/tokscale) v4.14.0. Codex tier evidence strategy is adapted from [`falyx6851-byte/codex-monitor`](https://github.com/falyx6851-byte/codex-monitor). Token Monitor adds encrypted cross-device ledgers, route provenance, subscription-equivalent accounting integration, and the dashboard.

See [`SOURCES.md`](SOURCES.md) and [`NOTICE`](NOTICE).

## License

MIT © Token Monitor contributors
