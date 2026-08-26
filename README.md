# Token Monitor

**Serverless, cross-device token and cost analytics for AI coding tools.**

Token Monitor is now a CLI-first project: one prebuilt Rust binary on Windows, Linux, or macOS, encrypted per-device snapshots in GitHub, and a static GitHub Pages analytics dashboard. No Electron, Node.js, Python, Docker, VPS, database, or resident Hub is required on user machines.

[中文说明](README.zh-CN.md)

## Why this fork

The original project already had excellent local usage parsing and visualization. This fork focuses on a different deployment problem: many developer machines should contribute to one usage view with essentially zero infrastructure and minimal background load.

```text
Windows ─┐
Linux ───┼─ local scan → AES-256-GCM snapshot → GitHub branches
macOS ───┘                                      ↓
                                      static GitHub Pages
                                      browser-local decrypt
```

Each device owns one `tm-ledger-<device-hash>` branch. A sync replaces that branch with a history-free snapshot, so telemetry does not flood `main` with commits and devices never contend on one shared file.

## Highlights

- One standalone Rust executable; users do **not** install Rust.
- No resident process: Task Scheduler / launchd / systemd user timer launches a short one-shot sync, then exits.
- Multi-client collection through pinned Tokscale Core: Codex, Claude Code, OpenCode, Gemini, Kimi and the broader client set Tokscale currently parses.
- Daily normalized rows keyed by device, client, upstream model vendor, route provider, route type, model and optional service tier.
- Conservative route attribution: inferred model vendor is not treated as proof of an official route.
- Distinguishes official APIs, AWS Bedrock, Azure OpenAI, Google Vertex, OpenRouter, New API / One API / LiteLLM / CLIProxyAPI-style relays, inference providers and self-hosted endpoints when source evidence supports it.
- Fast / standard / priority classification is recorded only when the source log exposes service-tier evidence.
- API-equivalent cost uses the canonical pricing engine; it is an estimate of API value, not a subscription invoice.
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

The installers download a platform-specific release binary only.

## First device

Use a public GitHub repository you can write to. Device ledgers are encrypted before upload, while public GitHub access lets the static dashboard read ciphertext without any browser-side GitHub credential.

```bash
token-monitor setup --repo YOUR_NAME/YOUR_REPO
```

The setup command validates repository access, performs the first full scan, installs the native OS timer, and prints a dashboard URL plus a Join Code.

## Additional devices

On a configured device:

```bash
token-monitor join
```

Then on another Windows / Linux / macOS machine:

```bash
token-monitor setup --join '<JOIN_CODE>'
```

Each machine supplies its own GitHub write credential. The Join Code carries the repository, dashboard decryption key and sync cadence.

## Commands

```text
token-monitor setup
token-monitor sync [--full]
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor join
token-monitor uninstall [--remove-remote] [--purge]
```

## Route identity model

The ledger keeps routing and model identity separate:

- `upstreamVendor`: model owner/family, e.g. OpenAI, Anthropic, Google.
- `routeProvider`: actual route/billing provider when evidenced, e.g. OpenAI, AWS Bedrock, Azure OpenAI, OpenRouter, New API.
- `routeType`: `official`, `cloud`, `aggregator`, `relay`, `inference-provider`, `self-hosted`, `custom`, or `unknown`.
- `provider`: the raw provider identifier retained for auditability.

This avoids a common analytics error: seeing a GPT model and automatically labeling the route “OpenAI official”. If only the model family is known, the upstream vendor can be OpenAI while the route remains `unknown`.

## Privacy

The GitHub snapshot contains only normalized aggregate rows and device metadata. Before upload it is encrypted with AES-256-GCM using a randomly generated 256-bit dashboard key. The dashboard key is placed after `#key=` in the dashboard URL; URL fragments are consumed locally by the browser and are not sent in HTTP requests to GitHub.

## Background load

There is no daemon. By default the OS scheduler runs one incremental sync every 15 minutes. Incremental sync rescans only a two-day overlap; full history is scanned on first setup or `token-monitor sync --full`.

## Dashboard

The static dashboard provides:

- Today / 7d / 30d / month / all / custom date range.
- Device, client, model, upstream vendor, route provider, route type, raw provider and service-tier filters.
- Total, input, cache read/write, output, reasoning, messages, sessions, duration and cost metrics.
- Line, area, bar, stacked bar, donut, treemap and table views.
- CSV export and light/dark themes.

## Verification

CI runs the Rust CLI tests and release-build smoke tests on Windows, macOS and Linux; JavaScript analytics tests; and the full pinned `tokscale-core` parser/scanner test suite. Release builds target Linux x86_64/aarch64, macOS x86_64/aarch64 and Windows x86_64/aarch64.

## Upstream and license

This repository is a fork/rewrite of [Javis603/token-monitor](https://github.com/Javis603/token-monitor) for a serverless multi-device use case and uses the Rust core from [junhoyeo/tokscale](https://github.com/junhoyeo/tokscale) as its multi-client scanning/pricing engine. The original MIT copyright notice is retained in `LICENSE`.
