# Security policy

## Supported versions

Iris is in public beta. Security fixes target the current `main` branch and the latest beta release. Older beta builds do not have a separate maintenance branch; update to a fixed release when one is available.

## Report privately

Use [GitHub's private vulnerability reporting form](https://github.com/UynajGI/Iris/security/advisories/new). Do not publish exploit details in a public issue before the maintainer has had an opportunity to investigate.

Include the affected version or commit, operating system, reproduction steps, expected security boundary and observed impact. Prefer a minimal synthetic sample. Remove private photographs, personal paths, database contents, access tokens and signing material.

Relevant areas include MCP directory restrictions, local daemon authentication, file export and quarantine, archive handling, model downloads and package integrity. Ordinary UI or compatibility problems belong in the [bug report form](https://github.com/UynajGI/Iris/issues/new?template=bug.yml).

The maintainer will coordinate investigation and disclosure through the private report. No response-time guarantee or bug bounty is offered.
