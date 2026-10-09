# Repository guidance

Read docs/HANDOFF.md for current implementation and validation boundaries.
Iris-owned source and documentation use GPL-3.0-or-later; preserve third-party notices.

- Keep UI work consistent with DESIGN.md. Use the locally installed Impeccable skill when available for design work; it is optional development tooling, not a repository dependency.
- Prefer shared core/daemon services over duplicating business logic in CLI, MCP or the frontend.
- Test risks and behavior changes; do not count ignored or unrun checks as passes.
- Never commit photographs, downloaded weights/runtimes, private databases, signing material, local agent tools or generated reports.
- Use disposable authorized photo copies for write/export/quarantine tests.
- Regenerate OpenAPI and frontend types when the contract changes.
- Run tools/check-public-tree.py after staging to check public-source boundaries and links.
- Do not publish, push, accept optional model licenses or change machine-wide settings without user authorization.
- If a local .codegraph index exists, use CodeGraph first for code exploration. Do not create an index automatically.
- Consult current SDK/library documentation for API-specific changes; use Context7 when available.
- Use context-mode for large output analysis when available; return compact findings.
