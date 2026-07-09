# Wildcard Support for @effect-diagnostics Directives

## Goal

Allow users to use `*` as a wildcard rule name in `@effect-diagnostics` directives to disable, suppress, or configure all Effect diagnostics at once, without listing every rule by name.

## Scope

- The directives parser in `internal/directives/parser.go`.
- The directive matching logic in `DirectiveSet`.
- Integration test coverage via baseline tests.

## Requirements

1. The directive regex must accept `*` as a valid rule name alongside alphanumeric rule names. A directive like `@effect-diagnostics *:off` or `@effect-diagnostics-next-line *:off` must be parsed successfully.

2. When matching rules, `*` must be treated as a wildcard that matches any rule name. The `*` wildcard replaces the previous `all` keyword — `all` must be removed from the matching logic. Only `*` is supported as a wildcard.

3. Wildcard directives must work in all directive positions:
   - Next-line: `// @effect-diagnostics-next-line *:off`
   - Section: `// @effect-diagnostics *:off`
   - File-level: `// @effect-diagnostics *:skip-file`

4. Wildcard follows the same section locality and precedence rules as any other directive — the most recent section directive (closest to the diagnostic, scanning upward) wins. This means:
   - A `*:off` section suppresses all rules from that point forward.
   - A subsequent `floatingEffect:error` section re-enables that specific rule from its position.
   - A subsequent `*:off` section suppresses everything again, overriding the rule-specific one.
   The key behavior is positional: whichever directive (wildcard or specific) appears closest above the diagnostic line takes effect for that rule.

5. Wildcard `*` next-line directives must not be tracked as "unused" for the unused-directive warning, since they apply to all rules and are inherently broad.

6. The disable quick fix must not emit wildcard directives — it continues to emit rule-specific `ruleName:off` comments. The wildcard is a user-authored convenience only.

## Non-Goals

- Glob patterns or partial wildcards (e.g., `floating*`). Only the literal `*` character is supported.

## Acceptance Criteria

1. `// @effect-diagnostics-next-line *:off` suppresses all Effect diagnostics on the following line.
2. `// @effect-diagnostics *:off` suppresses all Effect diagnostics from that point forward (section behavior).
3. `// @effect-diagnostics *:skip-file` skips the entire file for all rules.
4. A rule-specific override after a wildcard section takes effect: `*:off` followed by `floatingEffect:error` re-enables `floatingEffect` as error.
5. The directive parser unit tests cover wildcard parsing.
6. Baseline tests demonstrate wildcard suppression behavior.
