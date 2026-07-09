# CLI Platform Filesystem and Path Usage

## Goal
Ensure the TypeScript-Go CLI entrypoint in this project uses Effect Platform abstractions for filesystem and path behavior, rather than direct Node built-ins.

## Scope
- Applies to the language-service TypeScript-Go CLI behavior currently defined in `_packages/tsgo/src/cli.ts`.
- Covers all filesystem reads/writes/checks and path composition/normalization performed by the CLI.

## Requirements
1. CLI-visible behavior remains equivalent for users (inputs, outputs, error conditions, and exit behavior) after the change.
2. Filesystem interactions must be expressed through Effect Platform filesystem capabilities.
3. Path handling must be expressed through Effect Platform path capabilities.
4. The CLI must not depend on direct `node:fs` imports for its operational behavior.
5. The change must remain compatible with the project architecture rule that prefers Effect and shim-based integration boundaries.

## Non-Goals
- Introducing new CLI flags or changing documented CLI semantics.
- Expanding this requirement to unrelated packages unless explicitly requested.

## Acceptance Criteria
1. CLI commands complete the same workflows as before for valid and invalid inputs.
2. Filesystem/path-dependent flows continue to produce equivalent user-facing diagnostics and errors.
3. Runtime behavior is validated by the project’s required validation workflow with no regressions.
