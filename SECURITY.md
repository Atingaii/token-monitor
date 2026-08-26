# Security Policy

## Supported versions

Until the first stable release, security fixes are applied to the current development line and then included in the next release. After stable releases begin, this file will list the maintained release series explicitly.

## Reporting a vulnerability

Please do **not** publish sensitive details, credentials, decrypted ledgers, or real session data in a public issue.

For security-sensitive reports, use GitHub's private vulnerability reporting / Security Advisory flow for this repository when available. Include the minimum reproduction data needed to demonstrate the issue and redact all unrelated user content.

Security-relevant areas include:

- GitHub credential handling and storage;
- pair-code contents;
- AES-256-GCM ledger encryption/decryption;
- dashboard-key handling;
- accidental upload of raw prompts, responses, code, project paths, or auth files;
- malicious or untrusted ledger/dashboard input;
- release artifact integrity and checksum verification;
- privilege or command-injection issues in Task Scheduler, launchd, systemd, or cron integration.

## Data boundary

The intended remote ledger contains only normalized aggregate usage metrics, device metadata, and provider/routing labels. Raw prompt/response text, reasoning text, source code, project contents, project paths, full session files, API keys, GitHub tokens, and auth files must remain local.

A change that expands this remote data boundary should be treated as a security/privacy design change and reviewed explicitly.

## Credential model

GitHub credentials are local write credentials. They are not included in pair codes, encrypted ledgers, or dashboard URLs. Pair codes contain workspace information, dashboard decryption material, and synchronization configuration only.

## Cryptography

Device ledgers are encrypted using AES-256-GCM with a random dashboard key. The dashboard key is carried in the URL fragment (`#key=...`) so it is consumed by browser-side code rather than sent as part of the HTTP request path/query.

Cryptographic behavior should use established library primitives. Do not replace authenticated encryption with custom encryption, encoding, hashing, or obfuscation schemes.

## Release integrity

Release builds target the supported Windows/Linux/macOS architecture matrix. Installation scripts verify SHA-256 checksums before installing downloaded release archives.
