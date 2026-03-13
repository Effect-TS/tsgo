# Setup Command: Unified Change Pipeline

## Goal
Ensure the CLI `setup` command processes all file changes — including creation of new files — through a single, uniform pipeline: assess → compute changes → render diffs → confirm → apply. No file type should have a special-cased path in the orchestrator.

## Scope
- Applies to the setup command implementation in `_packages/tsgo/src/setup/`.
- Covers the orchestrator (`index.ts`), change computation (`changes.ts`), diff rendering (`diff-renderer.ts`), and the shared types (`types.ts`).

## Background
The reference implementation (`effect-language-service`) treats all file modifications uniformly as code actions with text changes. When a file doesn't exist yet (e.g., `.vscode/settings.json`), it uses a generic `isNewFile` mechanism: the code action's file change carries the full content, and the apply phase creates the directory and writes the file.

The current tsgo version deviates by special-casing VS Code settings creation: a separate `needsNewVSCodeSettings` flag, a dual exit condition, and a dedicated apply block that bypasses the change tracker, diff rendering, and user review.

## Reference Implementation
The canonical reference for this pattern lives in the `effect-language-service` repository (cloned at `.repos/effect-language-service`):

- **Orchestrator & apply loop**: `.repos/effect-language-service/packages/language-service/src/cli/setup.ts` — lines 185–234 contain the generic apply loop with `isNewFile` handling (mkdir + write for new files, reverse-order text change application for existing files).
- **Change computation**: `.repos/effect-language-service/packages/language-service/src/cli/setup/changes.ts` — `computeChanges()` aggregates changes and emits `{ fileName, textChanges, isNewFile }` for each file change.
- **Diff rendering**: `.repos/effect-language-service/packages/language-service/src/cli/setup/diff-renderer.ts` — `renderCodeActions()` handles both existing and new files uniformly.

## Requirements

1. The code action data model must support a marker indicating that a file change represents a new file (not a modification to an existing file).
2. When assessment detects that a target file does not exist but the user's target state requires it, `computeChanges()` must emit a code action with the new-file marker and the full file content as a text change — the same way it emits changes for existing files. Specifically: when the user selects a VS Code-based editor (VS Code, Cursor) and `.vscode/settings.json` does not exist, the pipeline must create it rather than silently skipping it.
3. The diff renderer must handle new-file code actions by showing the entire file content as additions (all green / `+` lines), so the user can review new file content before confirming.
4. The apply phase must handle new-file code actions generically: create the parent directory if needed and write the file content. This mechanism must not be specific to any single file type.
5. The orchestrator (`index.ts`) must not contain file-type-specific flags, exit conditions, or apply blocks. All file changes flow through the same pipeline phases.
6. User-visible behavior must remain equivalent: the same files are created/modified, the same prompts are shown, and the user can still review and confirm all changes before they are applied.

## Non-Goals
- Changing the assessment or target-prompt logic (what questions are asked, what options are available).
- Adding support for deleting files through the pipeline.
- Changing the diff rendering style or colors.

## Acceptance Criteria
1. Creating a new `.vscode/settings.json` (when the file doesn't exist) goes through the full pipeline: its content appears in the diff output, the user confirms, and then it's written.
2. The orchestrator has no special-cased flags or apply blocks for specific file types.
3. The apply loop handles both existing-file modifications and new-file creation through a single generic mechanism.
4. All existing setup scenarios (update existing files, add new dependencies, remove LSP, etc.) continue to work identically.
5. The project's validation workflow passes without regressions.
