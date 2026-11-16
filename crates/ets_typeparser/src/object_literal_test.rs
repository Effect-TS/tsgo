// Ported from effect-tsgo's internal/typeparser/object_literal_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_shim::scanner::get_text_of_node;

use crate::helpers_test::*;
use crate::module_identifier_test::parse_source;
use crate::*;

// go test: internal/typeparser/object_literal_test.go:10 TestObjectLiteralProperty c42a42db0ce2adb7
#[test]
fn test_object_literal_property() {
    run(|t| {
        t.parallel();
        const DECLARED: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags::Declared;
        const NOT_ASSIGNMENT: ObjectLiteralPropertyFlags =
            ObjectLiteralPropertyFlags::NotAssignment;
        const SPREAD: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags::MaybeSpread;
        const COMPUTED: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags::MaybeComputed;
        let declared_not_assignment = DECLARED | NOT_ASSIGNMENT;
        let declared_spread = DECLARED | SPREAD;
        let declared_computed = DECLARED | COMPUTED;
        let spread_computed = SPREAD | COMPUTED;
        let cases: [(&str, &str, &str, ObjectLiteralPropertyFlags); 20] = [
            ("absent", "{ discard: true }", "", ObjectLiteralPropertyFlags::None),
            ("empty", "{}", "", ObjectLiteralPropertyFlags::None),
            ("assignment", "{ concurrency: 4 }", "4", DECLARED),
            ("string key", r#"{ "concurrency": 4 }"#, "4", DECLARED),
            ("computed string key", r#"{ ["concurrency"]: 4 }"#, "4", DECLARED),
            ("computed template key", "{ [`concurrency`]: 4 }", "4", DECLARED),
            ("last assignment wins", "{ concurrency: 1, concurrency: 2 }", "2", DECLARED),
            ("under outer expressions", "({ concurrency: 4 } as const)", "4", DECLARED),
            ("under satisfies", "({ concurrency: 4 }) satisfies object", "4", DECLARED),
            ("shorthand", "{ concurrency }", "", declared_not_assignment),
            ("method", "{ concurrency() { return 1 } }", "", declared_not_assignment),
            ("assignment after shorthand", "{ concurrency, concurrency: 4 }", "4", DECLARED),
            ("spread only", "{ ...options }", "", SPREAD),
            ("spread after", "{ concurrency: 4, ...options }", "", declared_spread),
            ("spread before", "{ ...options, concurrency: 4 }", "4", DECLARED),
            ("computed only", "{ [key]: 4 }", "", COMPUTED),
            ("computed after", "{ concurrency: 4, [key]: 1 }", "", declared_computed),
            ("computed before", "{ [key]: 1, concurrency: 4 }", "4", DECLARED),
            ("spread and computed", "{ ...options, [key]: 1 }", "", spread_computed),
            ("not an object literal", "options", "", ObjectLiteralPropertyFlags::NotObjectLiteral),
        ];
        for (name, expression, want_initializer, want_flags) in cases {
            t.run(name, move |t| {
                t.parallel();
                let sf = parse_source(&format!(
                    "declare const options: object, key: string, concurrency: number\nconst x = {expression}"
                ));
                let statements = sf.statements().nodes();
                let declaration = statements[statements.len() - 1]
                    .as_variable_statement()
                    .declaration_list()
                    .as_variable_declaration_list()
                    .declarations()
                    .nodes()[0];
                let (initializer, flags) =
                    object_literal_property(declaration.initializer(), "concurrency");
                let got = initializer.map_or_else(String::new, get_text_of_node);
                if got != want_initializer || flags != want_flags {
                    t.error(sprintf!(
                        "ObjectLiteralProperty = (%q, %d), want (%q, %d)",
                        got,
                        flags.0,
                        want_initializer,
                        want_flags.0
                    ));
                }
                if initializer.is_some() != (flags == ObjectLiteralPropertyFlags::Declared) {
                    t.error(sprintf!(
                        "initializer %q with flags %d: an initializer must come with Declared alone",
                        got,
                        flags.0
                    ));
                }
                let wrapper = object_literal_property_initializer(declaration.initializer(), "concurrency");
                if wrapper != initializer {
                    t.error(sprintf!(
                        "ObjectLiteralPropertyInitializer = %v, want %v",
                        format!("{wrapper:?}"),
                        format!("{initializer:?}")
                    ));
                }
            });
        }
        let (initializer, flags) = object_literal_property(None, "concurrency");
        if initializer.is_some() || flags != ObjectLiteralPropertyFlags::NotObjectLiteral {
            t.error(sprintf!(
                "ObjectLiteralProperty(nil) = (%v, %d), want (nil, NotObjectLiteral)",
                format!("{initializer:?}"),
                flags.0
            ));
        }
    });
}
