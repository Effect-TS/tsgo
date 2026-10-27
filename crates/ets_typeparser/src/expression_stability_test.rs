use ets_go::sprintf;

use crate::helpers_test::*;
use crate::identity_forwarder_test::find_variable_initializer;

// go test: internal/typeparser/expression_stability_test.go:5 TestIsExpressionValueStableAtLocation
#[test]
fn test_is_expression_value_stable_at_location() {
    run(|t| {
        t.parallel();

        let source = r#"
enum Phase { Done = "done" }
declare const ambient: string
declare let ambientLet: string
const early = "early"
const plain = { value: 1 }
let mutable = 1
let PhaseAlias = Phase

const negative = -1
const negativeBig = -1n
const falseLiteral = false
const undefinedRef = undefined
const enumMember = Phase.Done
const ambientConst = ambient
const earlyConst = early
function nested() {
  const nestedRef = early
  return nestedRef
}

const positive = +1
const negatedIdentifier = -mutable
const sum = 1 + 1
const mutableRef = mutable
const ambientLetRef = ambientLet
const aliasedEnum = PhaseAlias.Done
const propertyRead = plain.value
function deferred() {
  const deferredRef = later
  return deferredRef
}
const laterEnumRef = LaterEnum.A
const laterAmbientRef = laterAmbient
const later = "later"
enum LaterEnum { A }
declare const laterAmbient: string
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let tests = [
            ("negative", true),
            ("negativeBig", true),
            ("falseLiteral", true),
            ("undefinedRef", true),
            ("enumMember", true),
            ("ambientConst", true),
            ("earlyConst", true),
            ("nestedRef", true),
            ("positive", false),
            ("negatedIdentifier", false),
            ("sum", false),
            ("mutableRef", false),
            ("ambientLetRef", false),
            ("aliasedEnum", false),
            ("propertyRead", false),
            ("deferredRef", false),
            ("laterEnumRef", false),
            ("laterAmbientRef", false),
        ];

        for (name, stable) in tests {
            // Rust: Go's nil check on the initializer is covered by find_variable_initializer, which fails the
            // test when the variable or its initializer is missing.
            let expression = find_variable_initializer(t, sf, name);
            let got = tp.is_expression_value_stable_at_location(expression, expression);
            if got != stable {
                t.error(sprintf!(
                    "IsExpressionValueStableAtLocation(%s) = %v, want %v",
                    name,
                    got,
                    stable
                ));
            }
        }
    });
}
