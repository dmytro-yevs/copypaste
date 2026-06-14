---
name: Security report
about: STOP — do not file security issues here
title: "[security]: please use the private advisory"
labels: security
---

> **Do not file public security issues.**
>
> Vulnerabilities in CopyPaste must be reported **privately** so we can ship a fix before disclosure.

## How to report

1. **Preferred — GitHub private advisory:**
   <https://github.com/dmytro-yevs/copypaste/security/advisories/new>

2. **Encrypted email:** `dmitriy.evseev.99@gmail.com`
   Encrypt with the maintainer's GPG key (fingerprint published in `SECURITY.md`).

## What to include

- Affected component (core / daemon / relay / Android / UI)
- Version or commit SHA
- Reproducer (PoC) — minimal and self-contained
- Impact (RCE, info disclosure, key leak, MITM, etc.)
- Suggested mitigation, if any

## Disclosure policy

- Acknowledgement within **72 hours**
- Fix target: **30 days** for high/critical, **90 days** for medium/low
- Coordinated disclosure preferred; credit given in the release notes unless you opt out

Thank you for helping keep CopyPaste users safe.
