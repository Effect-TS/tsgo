# Quick Fix Baselines

## Goal
Provide deterministic, human-readable baseline outputs that show (1) which quick fixes are available for each reported diagnostic and (2) what each quick fix changes when applied.

## Scope
- Applies to the Effect diagnostic baseline workflow under `testdata/baselines`.
- Covers baseline output contract for a new `*.quickfixes.txt` artifact.
- Covers multi-file test cases and diagnostics with multiple quick fixes.
- Covers quick-fix outputs for Effect V4 test baselines.
- Covers a minimal `fourslash` patch surface needed to list and apply quick fixes from tests.
- Covers a generic quick-fix baseline runner for both Effect V3 and Effect V4 test directories.

## Requirements
1. `*.errors.txt` formatting and content contract must remain unchanged.
2. A new `*.quickfixes.txt` baseline artifact must be generated for each test case alongside existing baseline artifacts.
3. The top section of `*.quickfixes.txt` must include, for each diagnostic occurrence:
- the stable diagnostic identifier,
- the diagnostic code and message,
- the list of available quick-fix titles in deterministic order.
4. The stable diagnostic identifier must be deterministic for a given test input and must distinguish repeated occurrences of the same diagnostic code in the same file.
5. `*.quickfixes.txt` must include an application-result entry for each available quick fix for each diagnostic occurrence.
6. Quick-fix application results in `*.quickfixes.txt` must be grouped by diagnostic occurrence and quick-fix index so every applied result is uniquely addressable.
7. For multi-file tests, quick-fix application output must clearly show which files changed for each applied quick fix.
8. For multi-file quick-fix results, changed files must be emitted in deterministic path order.
9. For each changed file, `*.quickfixes.txt` must include readable post-apply (`after`) output suitable for snapshot review.
10. If applying a quick fix creates or deletes files, `*.quickfixes.txt` must explicitly label file lifecycle events (created or deleted).
11. If a quick fix is available but produces no text edits for a diagnostic occurrence, `*.quickfixes.txt` must record it as a no-op result.
12. Baseline generation must preserve existing deterministic ordering guarantees by sorting diagnostics and quick-fix entries consistently.
13. The baseline contract must avoid coupling to implementation-specific internals; it must validate user-visible quick-fix titles and text-edit outcomes.
14. The baseline workflow must continue to pass the repository validation workflow without regressions to existing baseline artifacts.
15. Preferred delivery approach: add only the minimal methods needed in patched `fourslash` test APIs to (a) list quick fixes for a diagnostic context and (b) apply a selected quick fix.
16. Effect quick-fix baseline generation must use those `fourslash` methods in its runner flow rather than relying on the existing non-fourslash diagnostics runner implementation.
17. The quick-fix baseline generation must run through a generic fourslash-based runner that discovers all TypeScript test files in `testdata/tests/effect-v3` and `testdata/tests/effect-v4`, following the same per-version test-file discovery behavior used by the existing `*.errors.txt` baseline runner.
18. The generic runner must generate one `*.quickfixes.txt` baseline per discovered TypeScript test file in the corresponding versioned baseline output folder.
19. In the fourslash-based baseline flow, Effect-version-specific test library injection must be selected per test file, not through shared global process state.
20. In the fourslash-based baseline flow, a test file that starts with the marker comment `// @effect-v3` must be treated as an Effect v3 test for library injection.
21. In the fourslash-based baseline flow, if the v3 marker is not present, the runner must default that test file to Effect v4 library injection.
22. All fourslash test files under `testdata/tests/effect-v3` must include the `// @effect-v3` marker at the beginning of the file.
23. Disable-style quick fixes with titles matching `Disable ... for this line` or `Disable ... for entire file` must remain listed in the quick-fix inventory but must not emit file-change output in application results.
24. For those disable-style quick fixes, the application-result entry must be recorded as `skipped by default`.

## Non-Goals
- Defining a new language-service protocol.
- Changing quick-fix semantics or quick-fix title wording.
- Introducing requirements for non-Effect test suites.
- Large or unrelated patch expansion in `typescript-go`.

## Acceptance Criteria
1. `*.errors.txt` remains unchanged.
2. `*.quickfixes.txt` exists for baseline test cases and is deterministic across repeated runs.
3. The top section of `*.quickfixes.txt` contains diagnostic-indexed quick-fix inventory for all diagnostic occurrences in the test.
4. For diagnostics with multiple quick fixes, each fix has a distinct application-result entry (either applied output, no-op/create/delete result, or `skipped by default`).
5. Multi-file quick-fix application results clearly identify changed files and their post-apply (`after`) outputs.
6. No-op, create-file, delete-file, and `skipped by default` quick-fix outcomes are explicitly represented.
7. Existing and new baseline outputs are reviewable and sufficient to verify that current quick fixes match expected behavior.
8. The solution is delivered via minimal `fourslash` patch additions plus Effect test-suite output updates, without broader harness redesign.
9. A generic fourslash-based runner processes both Effect V3 and Effect V4 test folders and emits per-test `*.quickfixes.txt` outputs.
10. The fourslash-based baseline runs correctly inject Effect v3 libraries for marker-tagged tests and Effect v4 libraries for untagged tests.
