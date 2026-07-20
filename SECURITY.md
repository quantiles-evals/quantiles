# Security Policy

## Supported Versions

Quantiles provides security fixes for the latest released versions of actively maintained open source project components.

| Component                                     | Supported                                     |
| --------------------------------------------- | --------------------------------------------- |
| Latest `qt` CLI release                       | Yes                                           |
| Latest Python SDK release                     | Yes                                           |
| Older releases                                | No, unless explicitly stated in release notes |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues, discussions, pull requests, or other public forums.**

Instead, send vulnerability reports to security@quantiles.io.

Please include as much of the information below as you can. This will help us understand the nature and scope of the potential issue.

- A description of the vulnerability and likely impact
- Steps to reproduce, proof of concept, or affected command/API route
- Affected component(s), such as `qt`, the Python SDK, local `qt` server API, dataset loader, or export path
- Affected versions, commits, packages, or deployment environment
- Whether the issue involves secrets, authentication, local `.quantiles/` artifacts, or model outputs

Do not include real PHI, private customer datasets, secrets, API keys, access tokens, or full `.quantiles/` databases in vulnerability reports.

We aim to acknowledge vulnerability reports within three business days.

## Preferred Language

We prefer all communications to be in English.
