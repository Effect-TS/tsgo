use ets_etscore::{Severity, parse_severity};
use ets_go::sprintf;
use ets_shim::diagnostics;

use crate::*;

fn rs(rule: &str, severity: Severity) -> RuleSeverity {
    RuleSeverity { rule: rule.to_string(), severity }
}

fn directive(line: i64, is_next_line: bool, rules: Vec<RuleSeverity>) -> Directive {
    Directive { line, is_next_line, rules, ..Default::default() }
}

// go test: internal/directives/parser_test.go:10 TestEtscoreParseSeverity
#[test]
fn test_etscore_parse_severity() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, Severity)] = &[
            ("off", Severity::Off),
            ("OFF", Severity::Off),
            ("warning", Severity::Warning),
            ("warn", Severity::Warning),
            ("WARN", Severity::Warning),
            ("error", Severity::Error),
            ("ERROR", Severity::Error),
            ("suggestion", Severity::Suggestion),
            ("SUGGESTION", Severity::Suggestion),
            ("Suggestion", Severity::Suggestion),
            ("message", Severity::Message),
            ("MESSAGE", Severity::Message),
            ("Message", Severity::Message),
            ("skip-file", Severity::SkipFile),
            ("SKIP-FILE", Severity::SkipFile),
            ("unknown", Severity::Error), // default to error
            ("", Severity::Error),        // default to error
        ];

        for &(input, expected) in tests {
            t.run(input, |t| {
                t.parallel();
                let result = parse_severity(input);
                if result != expected {
                    t.error(sprintf!(
                        "etscore.ParseSeverity(%q) = %v, want %v",
                        input,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:46 TestEtscoreSeverityString
#[test]
fn test_etscore_severity_string() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(Severity, &str)] = &[
            (Severity::Off, "off"),
            (Severity::Warning, "warning"),
            (Severity::Error, "error"),
            (Severity::Suggestion, "suggestion"),
            (Severity::Message, "message"),
            (Severity::SkipFile, "skip-file"),
            (Severity(99), "unknown"),
        ];

        for &(severity, expected) in tests {
            t.run(expected, |t| {
                t.parallel();
                let result = severity.string();
                if result != expected {
                    t.error(sprintf!(
                        "etscore.Severity(%d).String() = %q, want %q",
                        severity.0,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:72 TestSeverityToCategory
#[test]
fn test_severity_to_category() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, Severity, diagnostics::Category)] = &[
            ("error maps to CategoryError", Severity::Error, diagnostics::Category::Error),
            ("warning maps to CategoryWarning", Severity::Warning, diagnostics::Category::Warning),
            (
                "suggestion maps to CategorySuggestion",
                Severity::Suggestion,
                diagnostics::Category::Suggestion,
            ),
            ("message maps to CategoryMessage", Severity::Message, diagnostics::Category::Message),
            ("off defaults to CategoryWarning", Severity::Off, diagnostics::Category::Warning),
            (
                "skip-file defaults to CategoryWarning",
                Severity::SkipFile,
                diagnostics::Category::Warning,
            ),
        ];

        for &(name, severity, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = to_category(severity);
                if result != expected {
                    t.error(sprintf!(
                        "ToCategory(%v) = %v, want %v",
                        severity,
                        format!("{result:?}"),
                        format!("{expected:?}")
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:122 TestCollectEffectDirectives
#[test]
fn test_collect_effect_directives() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: Vec<(&str, &str, Vec<Directive>)> = vec![
            ("no directives", "const x = 1;\nconst y = 2;", vec![]),
            (
                "single directive same line",
                "// @effect-diagnostics floatingEffect:off",
                vec![directive(0, false, vec![rs("floatingEffect", Severity::Off)])],
            ),
            (
                "next-line directive",
                "// @effect-diagnostics-next-line floatingEffect:off\nEffect.log(\"hello\")",
                vec![directive(0, true, vec![rs("floatingEffect", Severity::Off)])],
            ),
            (
                "multiple rules in one directive",
                "// @effect-diagnostics floatingEffect:off pocRule:warning",
                vec![directive(
                    0,
                    false,
                    vec![rs("floatingEffect", Severity::Off), rs("pocRule", Severity::Warning)],
                )],
            ),
            (
                "multiple directives on different lines",
                "// @effect-diagnostics floatingEffect:off\nconst x = 1;\n// @effect-diagnostics-next-line pocRule:warning",
                vec![
                    directive(0, false, vec![rs("floatingEffect", Severity::Off)]),
                    directive(2, true, vec![rs("pocRule", Severity::Warning)]),
                ],
            ),
            (
                "skip-file directive",
                "// @effect-diagnostics *:skip-file",
                vec![directive(0, false, vec![rs("*", Severity::SkipFile)])],
            ),
            (
                "suggestion severity directive",
                "// @effect-diagnostics-next-line floatingEffect:suggestion",
                vec![directive(0, true, vec![rs("floatingEffect", Severity::Suggestion)])],
            ),
            (
                "message severity directive",
                "// @effect-diagnostics-next-line floatingEffect:message",
                vec![directive(0, true, vec![rs("floatingEffect", Severity::Message)])],
            ),
            (
                "all severity levels in one directive",
                "// @effect-diagnostics rule1:error rule2:warning rule3:suggestion rule4:message",
                vec![directive(
                    0,
                    false,
                    vec![
                        rs("rule1", Severity::Error),
                        rs("rule2", Severity::Warning),
                        rs("rule3", Severity::Suggestion),
                        rs("rule4", Severity::Message),
                    ],
                )],
            ),
            (
                "wildcard section directive",
                "// @effect-diagnostics *:off",
                vec![directive(0, false, vec![rs("*", Severity::Off)])],
            ),
            (
                "wildcard next-line directive",
                "// @effect-diagnostics-next-line *:off",
                vec![directive(0, true, vec![rs("*", Severity::Off)])],
            ),
            (
                "mixed wildcard and rule-specific",
                "// @effect-diagnostics *:off floatingEffect:error",
                vec![directive(
                    0,
                    false,
                    vec![rs("*", Severity::Off), rs("floatingEffect", Severity::Error)],
                )],
            ),
            (
                "wildcard skip-file",
                "// @effect-diagnostics *:skip-file",
                vec![directive(0, false, vec![rs("*", Severity::SkipFile)])],
            ),
        ];

        for (name, source, expected) in &tests {
            t.run(name, |t| {
                t.parallel();
                let result = collect_effect_directives(source);
                if result.len() != expected.len() {
                    t.error(sprintf!(
                        "CollectEffectDirectives() returned %d directives, want %d",
                        result.len(),
                        expected.len()
                    ));
                    return;
                }
                for (i, d) in result.iter().enumerate() {
                    if d.line != expected[i].line {
                        t.error(sprintf!(
                            "directive[%d].Line = %d, want %d",
                            i,
                            d.line,
                            expected[i].line
                        ));
                    }
                    if d.is_next_line != expected[i].is_next_line {
                        t.error(sprintf!(
                            "directive[%d].IsNextLine = %v, want %v",
                            i,
                            d.is_next_line,
                            expected[i].is_next_line
                        ));
                    }
                    if d.rules.len() != expected[i].rules.len() {
                        t.error(sprintf!(
                            "directive[%d] has %d rules, want %d",
                            i,
                            d.rules.len(),
                            expected[i].rules.len()
                        ));
                        continue;
                    }
                    for (j, r) in d.rules.iter().enumerate() {
                        if r.rule != expected[i].rules[j].rule {
                            t.error(sprintf!(
                                "directive[%d].Rules[%d].Rule = %q, want %q",
                                i,
                                j,
                                r.rule,
                                expected[i].rules[j].rule
                            ));
                        }
                        if r.severity != expected[i].rules[j].severity {
                            t.error(sprintf!(
                                "directive[%d].Rules[%d].Severity = %v, want %v",
                                i,
                                j,
                                r.severity,
                                expected[i].rules[j].severity
                            ));
                        }
                    }
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:264 TestDirectiveAffectedLine
#[test]
fn test_directive_affected_line() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests = [
            ("same line directive", directive(5, false, vec![]), 5),
            ("next line directive", directive(5, true, vec![]), 6),
        ];

        for (name, directive, want_line) in &tests {
            t.run(name, |t| {
                t.parallel();
                let result = directive.affected_line();
                if result != *want_line {
                    t.error(sprintf!("Directive.AffectedLine() = %d, want %d", result, *want_line));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:294 TestDirectiveSetGetEffectiveSeverity
#[test]
fn test_directive_set_get_effective_severity() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r#"// @effect-diagnostics *:skip-file
// Normal code
// @effect-diagnostics-next-line floatingEffect:off
Effect.log("hello")
// @effect-diagnostics pocRule:warning
anotherCall()"#;

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str, i64, Severity, Severity)] = &[
            (
                "file-level skip applies to any rule",
                "anyRule",
                10,
                Severity::Error,
                Severity::SkipFile,
            ),
            (
                "next-line directive affects next line",
                "floatingEffect",
                3, // 0-indexed, so this is the 4th line "Effect.log(\"hello\")"
                Severity::Error,
                Severity::SkipFile, // file-level takes precedence
            ),
            (
                "same-line directive",
                "pocRule",
                4,
                Severity::Error,
                Severity::SkipFile, // file-level takes precedence
            ),
        ];

        for &(name, rule_name, line, default_severity, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity(rule_name, line, default_severity);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(%q, %d, %v) = %v, want %v",
                        rule_name,
                        line,
                        default_severity,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:348 TestDirectiveSetIsSuppressed
#[test]
fn test_directive_set_is_suppressed() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r#"// Line 0
// @effect-diagnostics-next-line floatingEffect:off
Effect.log("hello")  // Line 2
// Normal code       // Line 3"#;

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str, i64, bool)] = &[
            ("suppressed by next-line directive", "floatingEffect", 2, true),
            ("not suppressed - different rule", "pocRule", 2, false),
            ("not suppressed - different line", "floatingEffect", 3, false),
        ];

        for &(name, rule_name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.is_suppressed(rule_name, line);
                if result != expected {
                    t.error(sprintf!(
                        "IsSuppressed(%q, %d) = %v, want %v",
                        rule_name,
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:396 TestCaseInsensitiveRuleMatching
#[test]
fn test_case_insensitive_rule_matching() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics-next-line FloatingEffect:off
Effect.succeed(1)  // Line 1
// @effect-diagnostics-next-line POCRULE:warning
Effect.succeed(2)  // Line 3";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str, i64, Severity)] = &[
            ("lowercase rule matches PascalCase directive", "floatingeffect", 1, Severity::Off),
            ("PascalCase rule matches PascalCase directive", "FloatingEffect", 1, Severity::Off),
            ("camelCase rule matches UPPERCASE directive", "pocRule", 3, Severity::Warning),
            ("UPPERCASE rule matches UPPERCASE directive", "POCRULE", 3, Severity::Warning),
            ("different rule not affected", "otherRule", 1, Severity::Error),
        ];

        for &(name, rule_name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity(rule_name, line, Severity::Error);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(%q, %d, etscore.SeverityError) = %v, want %v",
                        rule_name,
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:456 TestSectionDirectives
#[test]
fn test_section_directives() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// Line 0 - normal code
Effect.succeed(1)  // Line 1 - no suppression
// @effect-diagnostics floatingEffect:off
Effect.succeed(2)  // Line 3 - suppressed by section
Effect.succeed(3)  // Line 4 - still suppressed
// @effect-diagnostics floatingEffect:warning
Effect.succeed(4)  // Line 6 - warning severity
Effect.succeed(5)  // Line 7 - still warning
Effect.succeed(6)  // Line 8 - still warning through EOF";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, i64, Severity)] = &[
            ("before any section directive", 1, Severity::Error),
            ("first section directive line", 3, Severity::Off),
            ("within first section", 4, Severity::Off),
            ("second section directive line", 6, Severity::Warning),
            ("within second section", 7, Severity::Warning),
            ("after last section directive (EOF)", 8, Severity::Warning),
        ];

        for &(name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity("floatingEffect", line, Severity::Error);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(\"floatingEffect\", %d, etscore.SeverityError) = %v, want %v",
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:520 TestSectionDirectiveAppliesByRuleUntilOverridden
#[test]
fn test_section_directive_applies_by_rule_until_overridden() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics floatingEffect:off
Effect.succeed(1)  // Line 1 - floatingEffect off
// @effect-diagnostics otherRule:warning
Effect.succeed(2)  // Line 3 - floatingEffect still off
Effect.succeed(3)  // Line 4 - floatingEffect still off
// @effect-diagnostics *:error
Effect.succeed(4)  // Line 6 - wildcard override error
Effect.succeed(5)  // Line 7 - wildcard override error";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str, i64, Severity)] = &[
            ("floatingEffect before unrelated section", "floatingEffect", 1, Severity::Off),
            ("floatingEffect after unrelated section", "floatingEffect", 3, Severity::Off),
            (
                "floatingEffect still off before wildcard override",
                "floatingEffect",
                4,
                Severity::Off,
            ),
            ("otherRule section applies", "otherRule", 3, Severity::Warning),
            ("wildcard override applies to floatingEffect", "floatingEffect", 6, Severity::Error),
            ("wildcard override applies to otherRule", "otherRule", 7, Severity::Error),
        ];

        for &(name, rule_name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity(rule_name, line, Severity::Error);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(%q, %d, etscore.SeverityError) = %v, want %v",
                        rule_name,
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:590 TestNextLineDirectiveTakesPrecedenceOverSection
#[test]
fn test_next_line_directive_takes_precedence_over_section() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics floatingEffect:off
Effect.succeed(1)  // Line 1 - suppressed by section
// @effect-diagnostics-next-line floatingEffect:error
Effect.succeed(2)  // Line 3 - error by next-line override
Effect.succeed(3)  // Line 4 - back to off from section";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, i64, Severity)] = &[
            ("section directive applies", 1, Severity::Off),
            ("next-line overrides section", 3, Severity::Error),
            ("back to section after next-line", 4, Severity::Off),
        ];

        for &(name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity("floatingEffect", line, Severity::Error);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(\"floatingEffect\", %d, etscore.SeverityError) = %v, want %v",
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:635 TestUsedDirectivesTracking
#[test]
fn test_used_directives_tracking() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics-next-line floatingEffect:off
Effect.succeed(1)  // Line 1 - will be checked
// @effect-diagnostics-next-line pocRule:off
Effect.succeed(2)  // Line 3 - different rule, won't suppress floatingEffect";

        let directives = collect_effect_directives(source);
        let mut ds = build_directive_set(&directives);

        // Check floatingEffect at line 1 - should suppress and mark used
        let sev1 = ds.get_effective_severity_and_mark_used("floatingEffect", 1, Severity::Error);
        if sev1 != Severity::Off {
            t.error(sprintf!(
                "Expected etscore.SeverityOff for floatingEffect at line 1, got %v",
                sev1
            ));
        }

        // Check floatingEffect at line 3 - pocRule directive shouldn't match
        let sev2 = ds.get_effective_severity_and_mark_used("floatingEffect", 3, Severity::Error);
        if sev2 != Severity::Error {
            t.error(sprintf!(
                "Expected etscore.SeverityError for floatingEffect at line 3, got %v",
                sev2
            ));
        }

        // Get unused directives
        let unused = ds.get_unused_next_line_directives(&directives);
        if unused.len() != 1 {
            t.error(sprintf!("Expected 1 unused directive, got %d", unused.len()));
        }
        if !unused.is_empty() && unused[0].line != 2 {
            t.error(sprintf!("Expected unused directive at line 2, got line %d", unused[0].line));
        }
    });
}

// go test: internal/directives/parser_test.go:667 TestWildcardSectionPrecedence
#[test]
fn test_wildcard_section_precedence() {
    ets_go::testing::run(|t| {
        t.parallel();
        // Mirrors the reference test: floatingEffect_disabledWildcard.ts
        // No directive → *:off suppresses → floatingEffect:error re-enables → *:off suppresses again
        let source = r"Effect.succeed(1)  // Line 0 - no directive, should fire
// @effect-diagnostics *:off
Effect.succeed(1)  // Line 2 - suppressed by wildcard
// @effect-diagnostics floatingEffect:error
Effect.succeed(1)  // Line 4 - re-enabled by rule-specific
// @effect-diagnostics *:off
Effect.succeed(1)  // Line 6 - suppressed again by wildcard";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, i64, Severity)] = &[
            ("before any directive - default severity", 0, Severity::Error),
            ("after *:off - suppressed", 2, Severity::Off),
            ("after floatingEffect:error - re-enabled", 4, Severity::Error),
            ("after second *:off - suppressed again", 6, Severity::Off),
        ];

        for &(name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity("floatingEffect", line, Severity::Error);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(\"floatingEffect\", %d, etscore.SeverityError) = %v, want %v",
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:721 TestWildcardOffThenRuleWarningReEnablesSpecificRule
#[test]
fn test_wildcard_off_then_rule_warning_re_enables_specific_rule() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r#"// @effect-diagnostics *:off
// @effect-diagnostics nodeBuiltinImport:warning
import fs from "node:fs""#;

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let got = ds.get_effective_severity("nodeBuiltinImport", 2, Severity::Error);
        if got != Severity::Warning {
            t.fatal(sprintf!(
                "GetEffectiveSeverity(nodeBuiltinImport, 2) = %v, want %v",
                got,
                Severity::Warning
            ));
        }

        let got = ds.get_effective_severity("floatingEffect", 2, Severity::Error);
        if got != Severity::Off {
            t.fatal(sprintf!(
                "GetEffectiveSeverity(floatingEffect, 2) = %v, want %v",
                got,
                Severity::Off
            ));
        }
    });
}

// go test: internal/directives/parser_test.go:739 TestWildcardOffAtTopLineStillLooksSuppressedAtLineZero
#[test]
fn test_wildcard_off_at_top_line_still_looks_suppressed_at_line_zero() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r#"// @effect-diagnostics *:off
// @effect-diagnostics nodeBuiltinImport:warning
import fs from "node:fs""#;

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        if !ds.is_suppressed("*", 0) {
            t.fatal("IsSuppressed(*, 0) = false, want true");
        }

        if ds.is_skip_file("*") {
            t.fatal("IsSkipFile(*) = true, want false");
        }
    });
}

// go test: internal/directives/parser_test.go:757 TestWildcardSkipFileIsDistinctFromOff
#[test]
fn test_wildcard_skip_file_is_distinct_from_off() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r#"// @effect-diagnostics *:skip-file
import fs from "node:fs""#;

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        if !ds.is_skip_file("*") {
            t.fatal("IsSkipFile(*) = false, want true");
        }

        if !ds.is_skip_file("nodeBuiltinImport") {
            t.fatal("IsSkipFile(nodeBuiltinImport) = false, want true");
        }
    });
}

// go test: internal/directives/parser_test.go:774 TestWildcardNextLineSuppression
#[test]
fn test_wildcard_next_line_suppression() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics-next-line *:off
Effect.succeed(1)  // Line 1 - suppressed by wildcard next-line
Effect.succeed(2)  // Line 2 - not suppressed";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str, i64, Severity)] = &[
            ("wildcard suppresses floatingEffect", "floatingEffect", 1, Severity::Off),
            ("wildcard suppresses any rule", "someOtherRule", 1, Severity::Off),
            ("next line not affected", "floatingEffect", 2, Severity::Error),
        ];

        for &(name, rule_name, line, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity(rule_name, line, Severity::Error);
                if result != expected {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(%q, %d, etscore.SeverityError) = %v, want %v",
                        rule_name,
                        line,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:821 TestWildcardSkipFile
#[test]
fn test_wildcard_skip_file() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics *:skip-file
Effect.succeed(1)  // Line 1 - skipped";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str)] = &[
            ("skips floatingEffect", "floatingEffect"),
            ("skips any rule", "anyOtherRule"),
            ("skips pocRule", "pocRule"),
        ];

        for &(name, rule_name) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.get_effective_severity(rule_name, 1, Severity::Error);
                if result != Severity::SkipFile {
                    t.error(sprintf!(
                        "GetEffectiveSeverity(%q, 1, etscore.SeverityError) = %v, want etscore.SeveritySkipFile",
                        rule_name,
                        result
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:850 TestWildcardUnusedDirectiveTracking
#[test]
fn test_wildcard_unused_directive_tracking() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics-next-line *:off
Effect.succeed(1)  // Line 1 - wildcard next-line
// @effect-diagnostics-next-line floatingEffect:off
Effect.succeed(2)  // Line 3 - specific rule next-line";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        // Don't mark any directive as used - neither should suppress a diagnostic
        // The wildcard directive should still NOT appear as unused
        let unused = ds.get_unused_next_line_directives(&directives);

        // Only the specific rule directive should be reported as unused, not the wildcard
        if unused.len() != 1 {
            t.fatal(sprintf!("Expected 1 unused directive, got %d", unused.len()));
        }
        if unused[0].line != 2 {
            t.error(sprintf!("Expected unused directive at line 2, got line %d", unused[0].line));
        }
        // Verify it's the floatingEffect one, not the wildcard
        if unused[0].rules[0].rule != "floatingEffect" {
            t.error(sprintf!(
                "Expected unused directive for floatingEffect, got %q",
                unused[0].rules[0].rule
            ));
        }
    });
}

// go test: internal/directives/parser_test.go:877 TestHasEnablingDirectiveWithWildcard
#[test]
fn test_has_enabling_directive_with_wildcard() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics *:error
Effect.succeed(1)";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        let tests: &[(&str, &str, bool)] = &[
            ("wildcard enables floatingEffect", "floatingEffect", true),
            ("wildcard enables any rule", "someOtherRule", true),
        ];

        for &(name, rule_name, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = ds.has_enabling_directive(rule_name);
                if result != expected {
                    t.error(sprintf!(
                        "HasEnablingDirective(%q) = %v, want %v",
                        rule_name,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:913 TestHasEnablingDirectiveWithWildcardOff
#[test]
fn test_has_enabling_directive_with_wildcard_off() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r"// @effect-diagnostics *:off
Effect.succeed(1)";

        let directives = collect_effect_directives(source);
        let ds = build_directive_set(&directives);

        // *:off should NOT count as enabling
        let result = ds.has_enabling_directive("floatingEffect");
        if result {
            t.error(sprintf!(
                "HasEnablingDirective(\"floatingEffect\") = %v, want false (wildcard off should not enable)",
                result
            ));
        }
    });
}

// go test: internal/directives/parser_test.go:928 TestHasAnyDirectiveForRule
#[test]
fn test_has_any_directive_for_rule() {
    ets_go::testing::run(|t| {
        t.parallel();

        let tests: &[(&str, &str, &str, bool)] = &[
            ("no directives", "Effect.succeed(1)", "floatingEffect", false),
            (
                "section directive mentions rule by name",
                "// @effect-diagnostics floatingEffect:error\nEffect.succeed(1)",
                "floatingEffect",
                true,
            ),
            (
                "section directive mentions other rule only",
                "// @effect-diagnostics pocRule:error\nEffect.succeed(1)",
                "floatingEffect",
                false,
            ),
            (
                "wildcard section directive mentions every rule",
                "// @effect-diagnostics *:error\nEffect.succeed(1)",
                "floatingEffect",
                true,
            ),
            (
                "lowering directive still counts as a mention",
                "// @effect-diagnostics floatingEffect:off\nEffect.succeed(1)",
                "floatingEffect",
                true,
            ),
            (
                "next-line directive counts as a mention",
                "// @effect-diagnostics-next-line floatingEffect:off\nEffect.log(\"x\")",
                "floatingEffect",
                true,
            ),
            (
                "skip-file directive counts as a mention",
                "// @effect-diagnostics floatingEffect:skip-file",
                "floatingEffect",
                true,
            ),
            (
                "rule name matching is case-insensitive",
                "// @effect-diagnostics FLOATINGeffect:error\nEffect.succeed(1)",
                "floatingEffect",
                true,
            ),
        ];

        for &(name, source, rule_name, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let ds = build_directive_set(&collect_effect_directives(source));
                let got = ds.has_any_directive_for_rule(rule_name);
                if got != expected {
                    t.error(sprintf!(
                        "HasAnyDirectiveForRule(%q) = %v, want %v",
                        rule_name,
                        got,
                        expected
                    ));
                }
            });
        }
    });
}
