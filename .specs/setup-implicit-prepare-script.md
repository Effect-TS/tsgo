# Setup Command: Implicit Prepare Script

## Goal
Remove the prepare script prompt from the setup command. The prepare script is now an implicit consequence of install/uninstall — not a user choice.

## Background
The Go-based language service (`@effect/tsgo`) uses a binary architecture. There is no "LSP-only" mode — the prepare script patches the TypeScript compiler to enable Effect diagnostics during compilation. Since the binary is all-or-nothing, asking whether to enable patching is unnecessary: if the user installs, they get the prepare script; if they uninstall, it's removed.

## Requirements

1. The setup command must not prompt the user about the prepare script.
2. When the user chooses to install the language service (any dependency type), `prepareScript` in the target state must be `true` automatically.
3. When the user chooses to uninstall the language service, `prepareScript` in the target state must be `false` automatically.
4. The `Target.PackageJson` type may keep the `prepareScript` field (it's still needed by `computeChanges()`), but it must be derived from the install/uninstall choice, not from user input.
5. After applying install changes, the setup command must display a message suggesting the user run `effect-tsgo patch` to complete the installation.
6. After applying uninstall changes, the setup command must display a message suggesting the user run `effect-tsgo unpatch` to restore the original TypeScript-Go binary.

## Non-Goals
- Changing which dependency type options are available (devDependencies, dependencies).
- Changing the editor selection prompt.

## Acceptance Criteria
1. The setup flow has one fewer prompt (no prepare script question).
2. Installing always adds the prepare script to `package.json`.
3. Uninstalling always removes the prepare script from `package.json`.
4. Post-apply output includes actionable next-step messages for both install and uninstall flows.
5. The project's validation workflow passes without regressions.
