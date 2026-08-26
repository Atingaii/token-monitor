# Security Policy

## Supported versions

Security fixes are applied to the current stable release line and included in the next patch release.

## Reporting a vulnerability

Please do **not** publish sensitive details, credentials, encrypted-ledger keys, decrypted private ledgers, or real session data in a public issue. Use GitHub's private vulnerability reporting / Security Advisory flow when available.

Security-relevant areas include:

- GitHub credential handling and storage;
- pair-code contents and workspace encryption-key handling;
- AES-256-GCM full-ledger encryption;
- accidental upload of raw prompts, responses, reasoning text, source code, project paths, auth files, API keys, or full sessions;
- accidental expansion of the public aggregate schema;
- malicious or untrusted `public.json` / `ledger.json` input;
- release artifact integrity and checksum verification;
- Task Scheduler, launchd, systemd, or cron command-injection / privilege issues.

## Remote data boundary

Token Monitor v1.0.1 intentionally writes **two representations** into each device-owned `tm-ledger-*` branch:

1. `public.json` — browser-readable aggregate statistics. This is intentionally public when the workspace fork is public and contains only dashboard fields such as date, device display metadata, client/model/provider/route/tier labels, additive token buckets, message counts, equivalent-cost estimates, and scan metadata.
2. `ledger.json` — the full aggregate ledger encrypted with AES-256-GCM. It is retained for encrypted workspace continuity and future private-mode use.

Neither representation may contain raw prompt/response text, reasoning text, source code, project contents, project paths, complete JSONL/SQLite sessions, API keys, GitHub tokens, `auth.json`, or other authentication material.

Because `public.json` is intentionally public, **do not use Token Monitor's public-dashboard mode if model/provider/device aggregate metadata itself is sensitive**. A future private-dashboard mode should use the encrypted ledger rather than weakening this boundary.

Any change that adds a field to `PublicLedger` is a security/privacy design change and must be reviewed explicitly.

## Credential model

GitHub credentials are local write credentials. They are never included in `public.json`, `ledger.json`, or Dashboard URLs. Pair codes contain the workspace repository, the shared ledger-encryption key, and synchronization cadence; they do **not** contain a GitHub credential.

## Cryptography

The encrypted full ledger uses AES-256-GCM with a random 256-bit workspace key. Cryptographic behavior must use established library primitives; do not replace authenticated encryption with custom encryption, encoding, hashing, or obfuscation.

The public Dashboard does not require that key because it reads only the sanitized `public.json` representation.

## Release integrity

Release builds cover Windows/Linux/macOS on x86_64 and ARM64. Installation scripts verify published SHA-256 checksums before installing downloaded archives.
