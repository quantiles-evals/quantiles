# Security Policy

## Supported Versions

Quantiles provides security fixes for the latest released versions of actively maintained project components.

| Component                                     | Supported                                     |
| --------------------------------------------- | --------------------------------------------- |
| Quantiles hosted services at `quantiles.io`   | Yes                                           |
| Latest `qt` CLI release                       | Yes                                           |
| Latest Python SDK release, `quantiles`        | Yes                                           |
| Latest TypeScript SDK release, if published   | Yes                                           |
| Older releases                                | No, unless explicitly stated in release notes |
| Archived directories or deprecated prototypes | No                                            |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues, discussions, or pull requests.**

Instead, please send them to help@quantiles.io.

Please include as much of the information below as you can. This will help us understand the nature and scope of the potential issue.

- A description of the vulnerability and likely impact
- Steps to reproduce, proof of concept, or affected command/API route
- Affected component, such as `qt`, the Python SDK, hosted web app, backend API, dataset loader, export path, or infrastructure configuration
- Affected versions, commits, packages, or deployment environment
- Whether the issue involves secrets, authentication, local `.quantiles/` artifacts, uploaded datasets, model outputs, PHI, or customer data
- Whether the issue has already been disclosed publicly

Do not include real PHI, private customer datasets, secrets, API keys, access tokens, or full `.quantiles/` databases in vulnerability reports.

We aim to acknowledge vulnerability reports within 3 business days.

## Preferred Language

We prefer all communications to be in English.
