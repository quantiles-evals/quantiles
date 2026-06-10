# Security Policy

## Supported Versions

Quantiles provides security fixes for the latest released versions of actively maintained project components.

| Component | Supported |
| --------- | --------- |
| Quantiles hosted services at `quantiles.io` | Yes |
| Latest `qt` CLI release | Yes |
| Latest Python SDK release, `quantiles` | Yes |
| Latest TypeScript SDK release, if published | Yes |
| Older releases | No, unless explicitly stated in release notes |
| Archived directories or deprecated prototypes | No |

## Reporting a Vulnerability

Please do not report security vulnerabilities through public GitHub issues, discussions, or pull requests.

To report a vulnerability, use this repository's private vulnerability reporting feature from the GitHub **Security** tab.

If private vulnerability reporting is unavailable, email:

help@quantiles.io

Please include:

- A description of the vulnerability and likely impact
- Steps to reproduce, proof of concept, or affected command/API route
- Affected component, such as `qt`, the Python SDK, hosted web app, backend API, dataset loader, export path, or infrastructure configuration
- Affected versions, commits, packages, or deployment environment
- Whether the issue involves secrets, authentication, local `.quantiles/` artifacts, uploaded datasets, model outputs, PHI, or customer data
- Whether the issue has already been disclosed publicly

## In Scope

Security issues in the following areas are in scope:

- The `qt` CLI and local Quantiles REST API
- The Python SDK under `sdk/src/quantiles`
- Quantiles workflow execution, step recording, resume behavior, metric recording, and export paths
- (PROBABLY NOT?) Local Quantiles workspace storage, including `.quantiles/quantiles.sqlite` and `.quantiles/metrics/`
- (PROBABLY NOT?) Dataset loading, caching, and export behavior that could expose private data or execute untrusted content
- (PROBABLY NOT SINCE THIS IS OPEN SOURCE REPO?) Hosted Quantiles web services, dashboard, documentation site, and API routes
- (PROBABLY NOT SINCE THIS IS OPEN SOURCE REPO?) Authentication, authorization, session handling, and Firebase/GCP integration
- (PROBABLY NOT SINCE THIS IS OPEN SOURCE REPO?) Cloud infrastructure, deployment configuration, GitHub Actions, release artifacts, and package publishing
- (PROBABLY NOT?) Secret handling for provider API keys, model credentials, service accounts, and cloud storage access
- (PROBABLY NOT?) Vulnerabilities that could expose customer data, private datasets, evaluation results, model outputs, PHI, or run artifacts

## Out of Scope

The following are usually out of scope unless they demonstrate a concrete security impact in Quantiles itself:

- General data quality issues in public benchmark datasets
- Model accuracy, benchmark scoring disagreements, hallucinations, or evaluation methodology disputes
- Vulnerabilities in third-party services or dependencies that are not caused or worsened by Quantiles
- Issues requiring access to a user's local machine, shell, cloud account, or credentials without a Quantiles vulnerability
- Expected network calls made by user workflows to remote model providers, hosted judges, external tools, or uncached dataset sources
- Coding-agent behavior outside Quantiles, including an agent misreading instructions, editing unrelated files, choosing unsafe shell commands, or running workflows the user did not intend, unless caused by a Quantiles vulnerability or unsafe default in Quantiles-provided agent instructions
- Unexpected model API, cloud, token, storage, or compute costs caused by user-configured benchmarks, provider settings, concurrency, sample size, hosted judges, external tools, or coding-agent retries, unless Quantiles bypasses configured limits or triggers unintended external calls
- Reports involving PHI or private customer data submitted to Quantiles without authorization or without an applicable agreement
- Denial-of-service reports based only on excessive local resource usage from intentionally large benchmarks

## Disclosure Process

We aim to acknowledge vulnerability reports within 3 business days.

After confirming a vulnerability, we will investigate the affected components and work on a fix. Depending on the issue, remediation may include a hosted service patch, CLI or SDK release, documentation update, GitHub security advisory, credential rotation, or infrastructure change.

Please allow a reasonable remediation period before public disclosure. 

## Data Handling

Do not include real PHI, private customer datasets, secrets, API keys, access tokens, or full `.quantiles/` databases in vulnerability reports.

If sensitive data is required to demonstrate impact, provide a minimal synthetic example or describe the data class involved. Quantiles services should not receive PHI unless a valid Business Associate Agreement is in place.

## Security Expectations for Users

Quantiles Open-source is local-first by default. Users are responsible for securing their own local workspaces, private datasets, model credentials, cloud storage, and workflow code.

Quantiles-managed local artifacts can contain sensitive evaluation data, including prompts, model outputs, metrics, sample-level results, and dataset-derived information. Treat `.quantiles/` directories as sensitive unless you know they contain only public or synthetic data.
