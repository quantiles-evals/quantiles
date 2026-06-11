# Contributing to Quantiles

Thanks for contributing to Quantiles!

Quantiles Open-source is a local-first evaluation infrastructure for AI systems. We welcome contributions that improve reliability, reproducibility, developer ergonomics, agent workflows, documentation, and benchmark and evaluation support.

## Ways to Contribute

Good contributions include:

- Bug fixes
- Documentation improvements
- Benchmark integrations
- CLI improvements
- SDK examples
- Runtime and storage improvements
- Tests and fixtures
- Agent workflow examples
- Improvements to `AGENTS.md`, `SKILL.md`, and agent-facing documentation

If you are not sure where to start, look for issues labeled:

- `good first issue`
- `help wanted`
- `documentation`
- `benchmark`
- `agent-workflow`
- `sdk`
- `cli`

## Before You Start

Before opening a pull request, please:

1. Search existing issues and pull requests to avoid duplicate work.
2. Open an issue for large changes, new benchmark integrations, schema changes, or API changes.
3. Keep changes focused and reviewable.
4. Update tests and documentation when behavior changes.
5. Follow the repository instructions in `AGENTS.md` when using a coding agent.

## Development Setup

Clone the repository:

```bash
git clone https://github.com/quantiles-evals/quantiles.git
cd quantiles
```

Install dependencies:

```bash
npm install
```

Initialize the local Quantiles workspace:

```bash
qt init
```

Run the test suite:

```bash
npm test
```

Run linting:

```bash
npm run lint
```

Run type checks:

```bash
npm run typecheck
```

If the package manager or commands differ in this repository, use the commands defined in `package.json`.

## Repository Structure

```text
quantiles/
├─ README.md
├─ CONTRIBUTING.md
├─ AGENTS.md
├─ llms.txt
│
├─ .github/
├─ packages/
├─ benchmarks/
├─ examples/
├─ docs/
├─ tests/
└─ scripts/
```

Important directories:

| Path | Purpose |
|---|---|
| `packages/` | CLI, core runtime, and SDK packages |
| `benchmarks/` | Built-in benchmark implementations and templates |
| `examples/` | Runnable examples for common evaluation workflows |
| `.agents/skills/` | Reusable agent skills for using Quantiles |
| `docs/` | Documentation for CLI, SDKs, benchmarks, agents, and reference material |
| `tests/` | CLI, SDK, runtime, storage, and benchmark tests |
| `.github/` | GitHub Actions, issue templates, and pull request templates |

## Pull Request Guidelines

A good pull request should:

- Solve one clear problem
- Include tests for new or changed behavior
- Update documentation when user-facing behavior changes
- Include clear before-and-after behavior when fixing a bug
- Avoid unrelated formatting or refactoring
- Keep generated files out of the diff unless they are required
- Explain any migration, compatibility, or benchmark comparability impact

Use a clear pull request title:

```text
fix(cli): handle missing run id in qt show
```

```text
docs(agents): clarify skill installation workflow
```

```text
feat(benchmarks): add simpleqa-verified run metadata
```

## Commit Style

Use concise, descriptive commit messages.

Preferred format:

```text
type(scope): summary
```

Examples:

```text
fix(cli): preserve exit code for failed workflows
```

```text
docs(quickstart): add sample-level inspection step
```

```text
test(runtime): cover resumed step execution
```

Common types include:

- `feat`
- `fix`
- `docs`
- `test`
- `refactor`
- `chore`
- `ci`

## Testing Requirements

Add or update tests when changing:

- CLI behavior
- Workflow execution
- Step caching or resume behavior
- Run metadata
- Storage schemas
- SDK APIs
- Benchmark scoring
- Metrics
- Artifact recording
- Error handling
- Comparison output

Before opening a pull request, run:

```bash
npm test
npm run lint
npm run typecheck
```

If your change affects a benchmark or example workflow, run the relevant workflow locally and inspect the recorded output:

```bash
qt run simpleqa-verified
qt show 1
```

When comparing behavior across changes, use:

```bash
qt compare <run_id_a> <run_id_b>
```

## Documentation Changes

Update documentation when changing:

- CLI commands or flags
- JSON output
- SDK APIs
- Workflow behavior
- Step caching behavior
- Resume behavior
- Benchmark inputs, outputs, scoring, or metrics
- Local state layout
- Agent instructions
- Installation or setup steps

Documentation should be:

- Clear
- Technical
- Reproducible
- Agent-friendly
- Easy to copy into a terminal or coding-agent prompt

Prefer concrete examples over abstract descriptions.

Good:

```bash
qt show <run_id>
```

Avoid vague instructions such as:

```text
inspect the results somehow
```

## Benchmark Contributions

Benchmark integrations must be reproducible and clearly documented.

When adding or changing a benchmark, include:

- Benchmark name
- Source dataset or repository
- License
- Dataset version or release
- File hashes when applicable
- Input and output shape
- Scoring method
- Metrics
- Known limitations
- Comparability notes
- Any required model, judge, tool, or external dependency
- Example command
- Tests or fixtures

Benchmark documentation should make it clear what the benchmark measures and what it does not measure.

If a benchmark uses an LLM judge, document:

- Judge model
- Prompt version
- Sampling settings
- Output schema
- Failure modes
- Any known sources of scoring variance

## SDK Contributions

When changing SDK behavior, document:

- Public API changes
- Backward compatibility impact
- Example usage
- Error behavior
- Runtime assumptions
- Serialization behavior
- How the SDK records inputs, outputs, metrics, artifacts, and metadata

SDK examples should be minimal, runnable, and easy to adapt.

## CLI Contributions

When changing CLI behavior, document:

- New or changed commands
- New or changed flags
- Exit codes
- JSON output changes
- Error messages
- Files written to local state
- Compatibility with existing runs

CLI output should be stable enough for both humans and coding agents to parse.

## Agent-Facing Contributions

Quantiles is designed to work well with coding agents.

When changing agent-facing files, make sure they are:

- Specific
- Durable
- Repository-aware
- Easy for agents to follow
- Grounded in actual CLI and SDK behavior

Agent-facing files include:

- [`SKILL.md`](https://github.com/quantiles-evals/skill/blob/main/SKILL.md)

`SKILL.md` gives agents durable Quantiles behavior for running evaluations, inspecting results, comparing runs, and summarizing regressions. To use the skill in another repository, copy the `quantiles` skill directory into that repository’s agent-supported skills directory.

Do not put project-specific implementation details into the reusable skill unless they apply to Quantiles usage across projects.

## Local State

Quantiles records evaluation state locally by default.

Do not commit local run state, generated caches, temporary artifacts, or machine-specific files unless they are intentional fixtures.

Before opening a pull request, check for accidental local files:

```bash
git status
```

If a fixture is required for a test, place it in the appropriate test fixture directory and document why it is needed.

## Security

Do not include secrets, API keys, tokens, credentials, private datasets, or private model outputs in issues, pull requests, tests, fixtures, examples, or documentation.

If you discover a security vulnerability, do not open a public GitHub issue.

See:

[SECURITY.md](./SECURITY.md)

## Code of Conduct

All contributors are expected to follow the project Code of Conduct.

[CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)

## Review Process

Maintainers may ask for:

- Smaller pull requests
- More tests
- Documentation updates
- Clearer benchmark provenance
- Compatibility notes
- Changes to naming, structure, or API design

Review is part of keeping Quantiles Open-source reliable and maintainable.

## Release Notes

Update `CHANGELOG.md` when a change affects:

- User-facing CLI behavior
- SDK APIs
- Benchmark behavior
- Scoring
- Metrics
- Run schemas
- Storage schemas
- Agent workflow instructions
- Documentation that changes expected usage

Use clear, technical language and include migration notes when needed.

## License

By contributing to Quantiles Open-source, you agree that your contributions will be licensed under the  [Apache License 2.0](./LICENSE).
