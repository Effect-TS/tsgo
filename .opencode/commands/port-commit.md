---
description: Explain how to port an effect-language-service commit into tsgo
---

Use `.repos/effect-language-service` as the source repository.
Update it to the latest commit in the origin/main branch before analyzing the reference commit.

Reference commit: `$1`

Follow the repository rules in @AGENTS.md.

Create a new branch for this porting work based of latest origin/main branch, e.g. `port-<feature-short-name>-<reference-commit-hash>`, and make sure to include the reference commit hash in the branch name for traceability.

Based on the reference commit above, port the change into this repository's Go implementation.

Requirements:
- Call out any TypeScript-only behavior that does not map directly to Go.
- If tests changed in the source commit, identify the corresponding tests or baselines to update here.
- Port added examples and tests
- Include validation steps appropriate for the touched files.

Output format:
1. Summary of the source commit.
2. Ported features and changes.
3. Output of the validation workflow.