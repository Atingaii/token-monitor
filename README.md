# Token Monitor

**Serverless, cross-device token and cost analytics for AI coding tools.**

Token Monitor is a CLI-first project: one prebuilt Rust binary on Windows, Linux, or macOS, encrypted per-device snapshots stored in the user's own Token Monitor fork, and one central static GitHub Pages analytics dashboard. No Electron, Node.js, Python, Docker, VPS, database, resident Hub, or second data repository is required.

[中文说明](README.zh-CN.md)

## Zero-infrastructure model

```text
Atingaii/token-monitor
        │ fork
        ▼
YOUR_NAME/token-monitor
  main                       = normal project source
  tm-ledger-<mac-hash>       = encrypted macOS snapshot
  tm-ledger-<win-hash>       = encrypted Windows snapshot
  tm-ledger-<linux-hash>     = encrypted Linux snapshot
        │
        ▼
Atingaii GitHub Pages dashboard
browser-local AES-GCM decryption
```

Each device owns one `tm-ledger-<device-hash>` branch. A sync force-moves only that device branch to a fresh root snapshot; `main` remains untouched and telemetry does not accumulate as a visible commit stream.

## Highlights

- One standalone Rust executable; users do **not** install Rust.
- No resident process: Task Scheduler / launchd / systemd user timer launches a short one-shot sync, then exits.
- Multi-client collection through pinned Tokscale Core: Codex, Claude Code, OpenCode, Gemini, Kimi and the broader client set Tokscale currently parses.
- Daily normalized rows keyed by device, client, upstream model vendor, route provider, route type, model and optional service tier.
- Conservative route attribution: inferred model vendor is not treated as proof of an official route.
- Distinguishes official APIs, AWS Bedrock, Azure OpenAI, Google Vertex, OpenRouter, New API / One API / LiteLLM / CLIProxyAPI-style relays, inference providers and self-hosted endpoints when source evidence supports it.
- Fast / standard / priority classification is recorded only when source logs expose service-tier evidence.
- Static dashboard with date/device/client/model/provider filters, line/area/bar/stacked-bar/donut/treemap/table views and CSV export.
- Uploads aggregate statistics only; never raw prompts, responses, reasoning text, source code, project content, API keys or full session transcripts.

## Install

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

## First device

```bash
token-monitor setup
```

That one command:

1. identifies the current GitHub account;
2. reuses `YOUR_NAME/token-monitor` if it is already a fork of this project;
3. otherwise attempts to create the fork automatically;
4. generates the dashboard encryption key;
5. scans local usage;
6. writes the first encrypted device snapshot;
7. installs the native low-overhead sync timer;
8. prints the dashboard URL and an exact command for the next device.

A renamed or organization-owned fork can still be selected explicitly with `--repo OWNER/RENAMED_FORK`.

## Additional devices

Setup prints a copy-paste command such as:

```bash
token-monitor join 'eyJ2ZXJzaW9uIjoyLC4uLn0'
```

Paste that single command on another Windows, Linux, or macOS machine. If you need it again later:

```bash
token-monitor invite
```

The pair code contains the fork address, dashboard decryption key and sync cadence. It deliberately does **not** contain a GitHub write token. Each device automatically checks `TOKEN_MONITOR_GITHUB_TOKEN`, `GITHUB_TOKEN`, `GH_TOKEN`, then an authenticated `gh auth token`; only if none exists does it ask once for a token with hidden terminal input.

## Commands

```text
token-monitor setup
token-monitor join <PAIR_CODE>
token-monitor invite
token-monitor sync [--full]
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor uninstall [--remove-remote] [--purge]
```

## Route identity model

The ledger keeps routing and model identity separate:

- `upstreamVendor`: model owner/family, e.g. OpenAI, Anthropic, Google.
- `routeProvider`: actual route/billing provider when evidenced, e.g. OpenAI, AWS Bedrock, Azure OpenAI, OpenRouter, New API.
- `routeType`: `official`, `cloud`, `aggregator`, `relay`, `inference-provider`, `self-hosted`, `custom`, or `unknown`.
- `provider`: raw provider identifier retained for auditability.

This avoids a common analytics error: seeing a GPT model and automatically labeling the route “OpenAI official”. If only the model family is known, the upstream vendor can be OpenAI while the route remains `unknown`.

## Privacy

The fork stores only normalized aggregate rows and device metadata, encrypted before upload with AES-256-GCM and a random 256-bit dashboard key. The dashboard key is carried after `#key=` in the URL; URL fragments are used locally by the browser and are not sent to GitHub with HTTP requests.

## Background load

There is no daemon. By default the OS scheduler runs one incremental sync every 15 minutes. Incremental sync rescans only a two-day overlap; full history is scanned on first setup or `token-monitor sync --full`.

## Dashboard

The central static dashboard provides:

- Today / 7d / 30d / month / all / custom date range.
- Device, client, model, upstream vendor, route provider, route type, raw provider and service-tier filters.
- Total, input, cache read/write, output, reasoning, messages, sessions, duration and cost metrics.
- Line, area, bar, stacked bar, donut, treemap and table views.
- CSV export and light/dark themes.

A user's fork does not need its own Pages configuration; the dashboard reads encrypted `tm-ledger-*` branches from the repository encoded in `?repo=OWNER/token-monitor`.

## Verification

CI validates installer syntax, Rust tests, release builds and CLI smoke tests on Windows, macOS and Linux; JavaScript analytics/privacy tests; and the full pinned `tokscale-core` parser/scanner suite. Release targets Linux x86_64/aarch64, macOS x86_64/aarch64 and Windows x86_64/aarch64.

## Upstream and license

This repository is a fork/rewrite of [Javis603/token-monitor](https://github.com/Javis603/token-monitor) for a serverless multi-device use case and uses the Rust core from [junhoyeo/tokscale](https://github.com/junhoyeo/tokscale) as its multi-client scanning/pricing engine. The original MIT copyright notice is retained in `LICENSE`.
