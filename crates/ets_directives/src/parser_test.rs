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

// go test: internal/directives/parser_test.go:10 TestEtscoreParseSeverity e89da4715cc6263c
#[test]
fn test_etscore_parse_severity() {
    ets_go::testing::run(|t| {
        t.parallel();
        const FALLBACK_SEVERITY: Severity = Severity::Error;
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
            ("unknown", FALLBACK_SEVERITY),
            ("", FALLBACK_SEVERITY),
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

// go test: internal/directives/parser_test.go:47 TestEtscoreSeverityString c722d5d306d123f1
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

// go test: internal/directives/parser_test.go:73 TestSeverityToCategory 6485eaa033419651
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

// go test: internal/directives/parser_test.go:123 TestCollectEffectDirectives ed7d7cd191a3d606
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

// go test: internal/directives/parser_test.go:258 TestDirectiveSetIsSuppressed 7db4802dc67297e5
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

// go test: internal/directives/parser_test.go:306 TestWildcardOffAtTopLineStillLooksSuppressedAtLineZero 742f80bdc8d15e5e
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

// go test: internal/directives/parser_test.go:324 TestWildcardSkipFileIsDistinctFromOff db983356390ea760
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

// go test: internal/directives/parser_test.go:341 TestHasEnablingDirective cbe1c055b8962f16
#[test]
fn test_has_enabling_directive() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str, bool)] = &[
            (
                "wildcard error enables floatingEffect",
                "// @effect-diagnostics *:error\nEffect.succeed(1)",
                "floatingEffect",
                true,
            ),
            (
                "wildcard error enables any rule",
                "// @effect-diagnostics *:error\nEffect.succeed(1)",
                "someOtherRule",
                true,
            ),
            (
                "wildcard off does not enable",
                "// @effect-diagnostics *:off\nEffect.succeed(1)",
                "floatingEffect",
                false,
            ),
        ];

        for &(name, source, rule_name, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let ds = build_directive_set(&collect_effect_directives(source));
                let got = ds.has_enabling_directive(rule_name);
                if got != expected {
                    t.error(sprintf!(
                        "HasEnablingDirective(%q) = %v, want %v",
                        rule_name,
                        got,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/directives/parser_test.go:363 TestHasAnyDirectiveForRule afea3778253a051f
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
