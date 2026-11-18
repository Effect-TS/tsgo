use ets_go::sprintf;

use crate::helpers_test::*;
use crate::identity_forwarder_test::find_variable_initializer;
use crate::*;

// go test: internal/typeparser/plain_parameter_test.go:5 TestIsPlainIdentifierParameter a9d8232680bdde2f
#[test]
fn test_is_plain_identifier_parameter() {
    run(|t| {
        t.parallel();

        let (_compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"
const plain = (value: number) => value
const optional = (value?: number) => value
const thisParameter = function (this: void) { return 1 }
const rest = (...values: Array<number>) => values
const defaulted = (value = 1) => value
const objectPattern = ({ value }: { value: number }) => value
const arrayPattern = ([value]: [number]) => value
",
        );

        // Rust: Go's map literal is a slice of pairs; the cases are independent, so order does not matter.
        for (name, want) in [
            ("plain", true),
            ("optional", true),
            ("thisParameter", true),
            ("rest", false),
            ("defaulted", false),
            ("objectPattern", false),
            ("arrayPattern", false),
        ] {
            let parameter = find_variable_initializer(t, sf, name).parameters()[0];
            let got = is_plain_identifier_parameter(parameter);
            if got != want {
                t.fatal(sprintf!("%s: IsPlainIdentifierParameter = %v, want %v", name, got, want));
            }
        }
        if is_plain_identifier_parameter(None) {
            t.fatal("expected nil to be rejected");
        }
        if is_plain_identifier_parameter(find_variable_initializer(t, sf, "plain")) {
            t.fatal("expected a non-parameter node to be rejected");
        }
    });
}
