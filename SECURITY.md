# Security Policy

## Supported versions

Security fixes are applied to the latest stable release and the current development line.

## Reporting a vulnerability

Do **not** publish credentials, decrypted compatibility ledgers, real session files, or private local data in a public issue. Use GitHub private vulnerability reporting / Security Advisory when available and include only the minimum redacted reproduction data required.

Security-sensitive areas include:

- GitHub credential handling and local storage;
- pair-code contents;
- AES-256-GCM compatibility-ledger encryption;
- accidental publication of raw prompts, responses, source code, project paths, session identifiers, auth files, API keys, or GitHub tokens;
- the public aggregate sanitization boundary;
- malicious/untrusted dashboard input;
- release artifact integrity and checksum verification;
- command or privilege issues in Task Scheduler, launchd, systemd, or cron integration.

## Remote data boundary

Token Monitor intentionally has **two** files in each history-free `tm-ledger-*` snapshot branch:

| File | Visibility / purpose |
| --- | --- |
| `ledger.json` | AES-256-GCM encrypted compatibility aggregate. Retained for migration/backward compatibility. |
| `public.json` | Public, de-identified aggregate used by the zero-login GitHub Pages dashboard. |

`public.json` is intentionally readable by anyone who can access the public fork. This is a product choice: the dashboard can therefore open in any browser without a remembered decryption key.

The public payload may contain only additive aggregate dimensions required by the dashboard:

- date;
- anonymous device hash/label, platform, architecture, app version;
- client, canonical model, upstream vendor, route provider/type and raw provider label;
- optional service tier;
- input, cache-read, cache-write, output and reasoning token buckets;
- normalized message count;
- API-equivalent and, where independently established, plan/legacy-meter equivalent cost estimates;
- lower-bound/availability flags and snapshot timing metadata.

The public payload must **never** contain:

- configured device name or hostname;
- original device ID;
- session IDs or full session records;
- prompts, assistant responses, or reasoning text;
- source code or project contents;
- project/workspace paths;
- `auth.json` or other auth files;
- API keys, GitHub tokens, cookies, passwords, or pair codes.

`PublicLedger::from_ledger` is the explicit sanitization boundary and has a regression test that proves local device identity fields are removed before serialization. Any change that expands `PublicLedger` or `UsageRow` should be reviewed as a privacy/security change.

## Credential model

GitHub credentials remain local write credentials. They are not included in `ledger.json`, `public.json`, or dashboard URLs. Pair codes contain the workspace repository, compatibility encryption material, and synchronization cadence; they never contain a GitHub credential.

## Cryptography

The compatibility ledger is encrypted using AES-256-GCM with a random 256-bit key and established library primitives. The key is retained for backward compatibility between joined devices, but the v1.0.1+ public dashboard does not require it.

Do not replace authenticated encryption with custom encryption, encoding, hashing, or obfuscation.

## Static-site password warning

A client-side password embedded in a public GitHub Pages application is **not** a meaningful protection boundary: both the application and public aggregate data are downloadable. If aggregate data must be private, use a private authenticated backend instead of adding a cosmetic JavaScript password gate.

## Release integrity

Release builds target Windows/Linux/macOS on x64 and ARM64. Installation scripts verify the platform release archive against its published SHA-256 checksum before installing it.
