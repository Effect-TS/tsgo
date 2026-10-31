use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;
use ets_shim::scanner::get_text_of_node;

use crate::helpers_test::*;

// go test: internal/typeparser/identity_forwarder_test.go:10 TestUnwrapIdentityForwarder 2f57adf131c8408c
#[test]
fn test_unwrap_identity_forwarder() {
    run(|t| {
        t.parallel();

        let source = r#"
declare const call: (value: unknown) => unknown
declare const other: (value: unknown) => unknown
declare const genericCall: <A>(value: unknown) => A
declare const source: Pipeable
interface Pipeable { pipe<B>(f: (self: Pipeable) => B): B; pipe<B, C>(f: (self: Pipeable) => B, g: (b: B) => C): C }

const direct = call
const arrow = (value: unknown) => call(value)
const arrowBlock = (value: unknown) => { return call(value) }
const functionExpression = function(value: unknown) { return call(value) }
const parenthesized = (value: unknown) => (call)((value))
const differentCallee = (value: unknown) => other(value)
const typeArguments = (value: unknown) => genericCall<string>(value)
const pipeable = (value: Pipeable) => value.pipe(call)

const wrapped = (value: unknown) => call(String(value))
const differentValue = (value: unknown) => call(undefined)
const shadowed = (value: unknown) => { const other = value; return call(other) }
const defaulted = (value: unknown = "fallback") => call(value)
const rest = (...values: Array<unknown>) => call(values)
const twoParameters = (value: unknown, other: unknown) => call(value)
const asynchronous = async (value: unknown) => call(value)
const generator = function*(value: unknown) { return call(value) }
const notACall = (value: unknown) => value
const pipeableChain = (value: Pipeable) => value.pipe(call, other)
const pipeableOtherSubject = (value: Pipeable) => source.pipe(call)
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        // name, wantTarget, wantUnwrap, wantTypeArguments
        let tests: &[(&str, &str, bool, usize)] = &[
            ("direct", "call", false, 0),
            ("arrow", "call", true, 0),
            ("arrowBlock", "call", true, 0),
            ("functionExpression", "call", true, 0),
            ("parenthesized", "call", true, 0),
            ("differentCallee", "other", true, 0),
            ("typeArguments", "genericCall", true, 1),
            ("pipeable", "call", true, 0),
            ("wrapped", "", false, 0),
            ("differentValue", "", false, 0),
            ("shadowed", "", false, 0),
            ("defaulted", "", false, 0),
            ("rest", "", false, 0),
            ("twoParameters", "", false, 0),
            ("asynchronous", "", false, 0),
            ("generator", "", false, 0),
            ("notACall", "", false, 0),
            ("pipeableChain", "", false, 0),
            ("pipeableOtherSubject", "", false, 0),
        ];

        for &(name, want_target, want_unwrap, want_type_arguments) in tests {
            let initializer = find_variable_initializer(t, sf, name);
            let original = ets_shim::ast::skip_parentheses(initializer);
            let (got, type_arguments, parameter) = tp.unwrap_identity_forwarder(initializer);
            if want_unwrap != parameter.is_some() {
                // Rust: Go's `%v` of the parameter node prints its address; its kind stands in for it.
                t.fatal(sprintf!(
                    "UnwrapIdentityForwarder(%s) returned parameter %v, want returned = %v",
                    name,
                    format!("{:?}", parameter.map(|p| p.kind())),
                    want_unwrap
                ));
            }
            let mut got_type_arguments = 0;
            if let Some(type_arguments) = type_arguments {
                got_type_arguments = type_arguments.nodes().len();
            }
            if got_type_arguments != want_type_arguments {
                t.fatal(sprintf!(
                    "UnwrapIdentityForwarder(%s) returned %d type arguments, want %d",
                    name,
                    got_type_arguments,
                    want_type_arguments
                ));
            }
            let got = got.expect("a non-nil node is returned for a non-nil input");
            if want_unwrap {
                if got == original {
                    t.fatal(sprintf!("UnwrapIdentityForwarder(%s) did not unwrap", name));
                }
                let got_text = get_text_of_node(got);
                if got_text != want_target {
                    t.fatal(sprintf!(
                        "UnwrapIdentityForwarder(%s) = %q, want %q",
                        name,
                        got_text,
                        want_target
                    ));
                }
            } else if got != original {
                t.fatal(sprintf!(
                    "UnwrapIdentityForwarder(%s) unexpectedly unwrapped to %q",
                    name,
                    get_text_of_node(got)
                ));
            }
        }
    });
}

// go test: internal/typeparser/identity_forwarder_test.go:99 findVariableInitializer d2ef79445e3972ba
pub(crate) fn find_variable_initializer(t: &T, sf: P<SourceFile>, name: &str) -> P<Node> {
    t.helper();

    fn walk(node: P<Node>, name: &str, result: &mut Option<P<Node>>) -> bool {
        if result.is_some() {
            return true;
        }
        if node.kind() == Kind::VariableDeclaration {
            let declaration = node.as_variable_declaration();
            let declaration_name = declaration.name();
            if declaration_name.kind() == Kind::Identifier && declaration_name.text() == name {
                *result = declaration.initializer();
                return true;
            }
        }
        node.for_each_child(&mut |child| walk(child, name, result));
        false
    }
    let mut result = None;
    walk(sf.as_node(), name, &mut result);
    let Some(result) = result else {
        t.fatal(sprintf!("variable initializer %q not found", name));
    };
    result
}
