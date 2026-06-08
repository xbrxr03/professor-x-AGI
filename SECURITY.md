# Security Policy

## Supported Versions

Professor X is pre-1.0 research software. Security fixes are applied to the `main` branch only.

## Reporting a Vulnerability

**Do not file public issues for security vulnerabilities.**

Instead, please report security issues by:

1. **Email**: Send details to the repository maintainer
2. **GitHub**: Use the [privately report a vulnerability](../../security/advisories/new) feature

Please include:
- The component affected (e.g., `policyd`, `toolbridge`, `vault`)
- Steps to reproduce
- Potential impact
- Suggested fix (if you have one)

## Security Architecture

Professor X includes several built-in safety mechanisms:

- **Policy Gate** (`policyd`) — All tool calls pass through a risk-scoring engine. Operations with risk ≥ 65 are queued for human approval.
- **Credential Vault** — AES-256-GCM encrypted storage. Credentials never appear in LLM prompts, audit logs, or evolution diffs.
- **Audit Chain** — Every autonomous action is recorded with a content hash chain.
- **Workspace Boundaries** — The agent cannot write outside its designated workspace root.
- **Blocked Paths** — Vault keys, system files, and cloud metadata endpoints are explicitly blocked.
- **Kill Switch** — SIGUSR2 for graceful shutdown; Ctrl+C for foreground processes.

## Responsible Disclosure

We ask that you:
- Give us 90 days to address the issue before public disclosure
- Avoid accessing or modifying other users' data
- Do not degrade the service for other researchers

We will:
- Acknowledge your report within 48 hours
- Provide a timeline for the fix
- Credit you in the advisory (unless you prefer anonymity)

Thank you for helping keep Professor X safe.