# GitHub Ledger Sync (experimental)

This fork adds a serverless sync provider for cross-device Token Monitor usage.

## Architecture

Each Windows, Linux, or macOS device keeps collecting usage locally with the existing Token Monitor collector. Instead of POSTing the summary to a running Hub, the agent can write its latest normalized summary to a GitHub repository:

```text
Windows ─┐
Linux ───┼─> GitHub repository / ledger/devices/<device>.json
macOS ───┘
                      ↓
               GitHub Pages
               /usage.html
```

The ledger stores Token Monitor's normalized summary, not raw Codex/Claude session files.

## Agent configuration

Set these environment variables on each device:

```bash
TOKEN_MONITOR_SYNC_PROVIDER=github
TOKEN_MONITOR_GITHUB_REPO=OWNER/LEDGER_REPOSITORY
TOKEN_MONITOR_GITHUB_TOKEN=YOUR_FINE_GRAINED_PAT
TOKEN_MONITOR_GITHUB_BRANCH=main
TOKEN_MONITOR_GITHUB_BASE_PATH=ledger/devices
TOKEN_MONITOR_DEVICE_ID=my-device-name
```

Then run the normal agent:

```bash
npm run agent
```

For a one-shot test:

```bash
npm run agent:once
```

The token should have Contents read/write access only to the selected ledger repository. Do not commit the token to `.env`, source control, screenshots, or the public dashboard.

## Ledger layout

```text
ledger/
└── devices/
    ├── macbook-pro.json
    ├── windows-workstation.json
    └── linux-server.json
```

Each device owns one path, which avoids normal multi-writer conflicts.

## Dashboard

The GitHub Pages build publishes:

```text
/usage.html
```

For a **public ledger repository**, open:

```text
https://<owner>.github.io/<repo>/usage.html?repo=OWNER/LEDGER_REPOSITORY
```

The page reads the public GitHub Contents API, loads every device ledger, and aggregates:

- total tokens
- API-equivalent cost
- cache-read tokens
- output tokens
- per-device totals
- per-model totals
- Today / Month / All time

## Privacy boundary

The current MVP dashboard intentionally does **not** accept or persist a GitHub token in the browser. Therefore it can directly read only a public ledger repository.

The intended next stage is a private-ledger publishing pipeline:

```text
Private ledger repository
        ↓
GitHub Actions sanitizer / aggregator
        ↓
public or AES-GCM-encrypted dashboard artifact
        ↓
GitHub Pages
```

This keeps detailed device ledgers private while still allowing a static Pages dashboard.

## Compatibility

The original Hub transport remains the default. Existing users do not need to change anything:

```text
TOKEN_MONITOR_SYNC_PROVIDER=hub
```

or omit `TOKEN_MONITOR_SYNC_PROVIDER` entirely.
