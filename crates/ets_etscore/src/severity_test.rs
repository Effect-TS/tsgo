use ets_go::sprintf;

use crate::*;

// go test: etscore/severity_test.go:5 TestSeverityAtLeastAsVisibleAs 2bc2465e86537c77
#[test]
fn test_severity_at_least_as_visible_as() {
    ets_go::testing::run(|t| {
        t.parallel();

        let tests: &[(&str, Severity, Severity, bool)] = &[
            ("error meets warning threshold", Severity::Error, Severity::Warning, true),
            ("warning meets warning threshold", Severity::Warning, Severity::Warning, true),
            ("suggestion below warning threshold", Severity::Suggestion, Severity::Warning, false),
            ("message below warning threshold", Severity::Message, Severity::Warning, false),
            ("off below warning threshold", Severity::Off, Severity::Warning, false),
            ("skip-file below warning threshold", Severity::SkipFile, Severity::Warning, false),
            ("error meets message threshold", Severity::Error, Severity::Message, true),
            ("warning meets message threshold", Severity::Warning, Severity::Message, true),
            ("suggestion meets message threshold", Severity::Suggestion, Severity::Message, true),
            ("message meets message threshold", Severity::Message, Severity::Message, true),
            ("off below message threshold", Severity::Off, Severity::Message, false),
            ("suggestion below error threshold", Severity::Suggestion, Severity::Error, false),
            ("warning below error threshold", Severity::Warning, Severity::Error, false),
            ("error meets error threshold", Severity::Error, Severity::Error, true),
        ];

        for &(name, severity, min, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let got = severity.at_least_as_visible_as(min);
                if got != expected {
                    t.error(sprintf!(
                        "%v.AtLeastAsVisibleAs(%v) = %v, want %v",
                        severity,
                        min,
                        got,
                        expected
                    ));
                }
            });
        }
    });
}
