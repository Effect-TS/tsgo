use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::module_identifier_test::parse_source;
use crate::*;

/// Rust-only: Go's `(*TypeParser)(nil)` receiver. The Rust methods take `&mut self`, so the parses run on a parser
/// over an empty program, which they never consult: the fixtures hold no `Predicate.isTagged` candidate, the only
/// checker-backed recognition of these parses (Go's nil parser would dereference nil there).
fn nil_type_parser_program(t: &T) -> Compiled {
    let (compiled, _) = compile_and_get_checker_and_source_file_internal(t, "");
    compiled
}

// go test: internal/typeparser/returning_dispatch_test.go:12 TestParseReturningDispatchAcceptedShapes
// Rust: Go's `dispatch.body == nil`, `dispatch.dispatch == nil` and `branch.condition.source == nil` checks drop:
// the body and the source are `P<Node>`s and the dispatch is held by value, never nil. Go prints the dispatch with `%+v` (pointer addresses); its
// `Debug` form stands in.
#[test]
fn test_parse_returning_dispatch_accepted_shapes() {
    struct Case {
        name: &'static str,
        function: &'static str,
        tests: Vec<&'static str>,
        discriminants: Vec<&'static str>,
        results: Vec<&'static str>,
        fallback: &'static str,
    }
    run(|t| {
        t.parallel();
        let tests = vec![
            Case {
                name: "arrow conditional",
                function: "(value: unknown) => isA(value) ? first : fallback",
                tests: vec!["isA(value)"],
                discriminants: vec![],
                results: vec!["first"],
                fallback: "fallback",
            },
            Case {
                name: "returned conditional chain",
                function: "(value: unknown) => {
				return isA(value) ? first : isB(value) ? second : fallback
			}",
                tests: vec!["isA(value)", "isB(value)"],
                discriminants: vec![],
                results: vec!["first", "second"],
                fallback: "fallback",
            },
            Case {
                name: "function expression if with trailing fallback",
                function: "function(value: unknown) {
				if (isA(value)) return first
				return fallback
			}",
                tests: vec!["isA(value)"],
                discriminants: vec![],
                results: vec!["first"],
                fallback: "fallback",
            },
            Case {
                name: "switch exposes discriminant and case expressions",
                function: r#"(value: unknown) => {
				switch (classify(value)) {
					case "A": return first
					case "B": return second
					default: return fallback
				}
			}"#,
                tests: vec![r#""A""#, r#""B""#],
                discriminants: vec!["classify(value)", "classify(value)"],
                results: vec!["first", "second"],
                fallback: "fallback",
            },
        ];

        for tt in tests {
            t.run(tt.name, |t| {
                t.parallel();
                let (sf, function) = parse_returning_dispatch_test_function(t, tt.function);
                let mut nil_program = nil_type_parser_program(t);
                let Some(dispatch) = nil_program.type_parser().parse_returning_dispatch(function)
                else {
                    t.fatal("expected returning dispatch");
                };
                if dispatch.node != function || dispatch.params.map_or(0, <[_]>::len) != 1 {
                    t.fatal(format!("unexpected function evidence: {dispatch:?}"));
                }
                let result_dispatch = &dispatch.dispatch;
                if result_dispatch.node != dispatch.body {
                    t.fatal("result dispatch node does not preserve the function body");
                }

                let mut tests = vec![String::new(); result_dispatch.branches.len()];
                let mut discriminants = vec![String::new(); result_dispatch.branches.len()];
                let mut results = vec![String::new(); result_dispatch.branches.len()];
                for (index, branch) in result_dispatch.branches.iter().enumerate() {
                    let mut test = Some(branch.condition.subject);
                    if branch.condition.kind == DispatchConditionKind::SwitchCase {
                        discriminants[index] =
                            returning_dispatch_node_text(Some(sf), Some(branch.condition.subject));
                        test = branch.condition.value;
                    }
                    tests[index] = returning_dispatch_node_text(Some(sf), test);
                    results[index] = returning_dispatch_node_text(Some(sf), Some(branch.result));
                }
                if tests != tt.tests {
                    t.fatal(sprintf!("tests = %v, want %v", tests, tt.tests));
                }
                if results != tt.results {
                    t.fatal(sprintf!("results = %v, want %v", results, tt.results));
                }
                if !tt.discriminants.is_empty() && discriminants != tt.discriminants {
                    t.fatal(sprintf!(
                        "discriminants = %v, want %v",
                        discriminants,
                        tt.discriminants
                    ));
                }
                let got = returning_dispatch_node_text(Some(sf), result_dispatch.fallback);
                if got != tt.fallback {
                    t.fatal(sprintf!("fallback = %q, want %q", got, tt.fallback));
                }
            });
        }
    });
}

// go test: internal/typeparser/returning_dispatch_test.go:111 TestParseResultDispatchInputContract
// Rust: Go's `branch.condition.subject == nil` check drops (the subject is a `P<Node>`, never nil). Go prints the
// dispatch with `%+v` (pointer addresses); its `Debug` form stands in.
#[test]
fn test_parse_result_dispatch_input_contract() {
    struct Case {
        name: &'static str,
        expression: &'static str,
        branches: Vec<&'static str>,
        fallback: &'static str,
        accepted: bool,
    }
    run(|t| {
        t.parallel();
        let tests = vec![
            Case {
                name: "conditional expression",
                expression: "isA(value) ? first : fallback",
                branches: vec!["isA(value)"],
                fallback: "fallback",
                accepted: true,
            },
            Case {
                name: "false branch chain is flattened",
                expression: "isA(value) ? first : isB(value) ? second : fallback",
                branches: vec!["isA(value)", "isB(value)"],
                fallback: "fallback",
                accepted: true,
            },
            Case {
                name: "true branch nesting is rejected",
                expression: "isA(value) ? (isB(value) ? first : second) : fallback",
                branches: vec![],
                fallback: "",
                accepted: false,
            },
            Case {
                name: "plain expression is rejected",
                expression: "first",
                branches: vec![],
                fallback: "",
                accepted: false,
            },
        ];

        for tt in tests {
            t.run(tt.name, |t| {
                t.parallel();
                let (sf, function) =
                    parse_returning_dispatch_test_function(t, &format!("() => {}", tt.expression));
                let body = get_function_like_body(function);
                let mut nil_program = nil_type_parser_program(t);
                let dispatch = nil_program.type_parser().parse_result_dispatch(body);
                if !tt.accepted {
                    if let Some(dispatch) = dispatch {
                        t.fatal(format!("expected parse rejection, got {dispatch:?}"));
                    }
                    return;
                }
                let Some(dispatch) =
                    dispatch.as_ref().filter(|dispatch| Some(dispatch.node) == body)
                else {
                    t.fatal(format!("expected result dispatch for body, got {dispatch:?}"));
                };
                let mut conditions = vec![String::new(); dispatch.branches.len()];
                for (index, branch) in dispatch.branches.iter().enumerate() {
                    if branch.condition.kind != DispatchConditionKind::Predicate
                        || branch.condition.value.is_some()
                    {
                        t.fatal(sprintf!("branch %d is not a predicate", index as i64));
                    }
                    conditions[index] =
                        returning_dispatch_node_text(Some(sf), Some(branch.condition.subject));
                }
                if conditions != tt.branches {
                    t.fatal(sprintf!("branches = %v, want %v", conditions, tt.branches));
                }
                let got = returning_dispatch_node_text(Some(sf), dispatch.fallback);
                if got != tt.fallback {
                    t.fatal(sprintf!("fallback = %q, want %q", got, tt.fallback));
                }
            });
        }
    });
}

// go test: internal/typeparser/returning_dispatch_test.go:178 TestParseResultDispatchTagHints
#[test]
fn test_parse_result_dispatch_tag_hints() {
    struct Case {
        name: &'static str,
        function: &'static str,
        tag_subjects: Vec<&'static str>,
        tag_values: Vec<&'static str>,
    }
    run(|t| {
        t.parallel();
        let tests = vec![
            Case {
                name: "tag equality predicate",
                function: r#"(value: unknown) => value._tag === "Some" ? first : fallback"#,
                tag_subjects: vec!["value"],
                tag_values: vec![r#""Some""#],
            },
            Case {
                name: "reversed tag equality predicate",
                function: r#"(error: unknown) => "ReasonA" == error.reason._tag ? first : fallback"#,
                tag_subjects: vec!["error.reason"],
                tag_values: vec![r#""ReasonA""#],
            },
            Case {
                name: "tag switch cases",
                function: r#"(value: unknown) => {
				switch (value._tag) {
					case "Some": return first
					case "None": return second
					default: return fallback
				}
			}"#,
                tag_subjects: vec!["value", "value"],
                tag_values: vec![r#""Some""#, r#""None""#],
            },
            Case {
                name: "non-literal tag value remains source evidence",
                function: "(value: unknown) => value._tag === expectedTag ? first : fallback",
                tag_subjects: vec!["value"],
                tag_values: vec!["expectedTag"],
            },
            Case {
                name: "non-tag predicate has no hints",
                function: r#"(value: unknown) => value.kind === "Some" ? first : fallback"#,
                tag_subjects: vec![""],
                tag_values: vec![""],
            },
            Case {
                name: "non-equality tag predicate has no hints",
                function: r#"(value: unknown) => value._tag !== "Some" ? first : fallback"#,
                tag_subjects: vec![""],
                tag_values: vec![""],
            },
        ];

        for tt in tests {
            t.run(tt.name, |t| {
                t.parallel();
                let (sf, function) = parse_returning_dispatch_test_function(t, tt.function);
                let mut nil_program = nil_type_parser_program(t);
                let Some(parsed) = nil_program.type_parser().parse_returning_dispatch(function)
                else {
                    t.fatal("expected returning dispatch");
                };
                if parsed.dispatch.branches.len() != tt.tag_subjects.len()
                    || tt.tag_subjects.len() != tt.tag_values.len()
                {
                    t.fatal(sprintf!(
                        "branches = %d, want %d",
                        parsed.dispatch.branches.len() as i64,
                        tt.tag_subjects.len() as i64
                    ));
                }
                for (index, branch) in parsed.dispatch.branches.iter().enumerate() {
                    let got = returning_dispatch_node_text(Some(sf), branch.condition.tag_subject);
                    if got != tt.tag_subjects[index] {
                        t.fatal(sprintf!(
                            "branch %d tag subject = %q, want %q",
                            index as i64,
                            got,
                            tt.tag_subjects[index]
                        ));
                    }
                    let got = returning_dispatch_node_text(Some(sf), branch.condition.tag_value);
                    if got != tt.tag_values[index] {
                        t.fatal(sprintf!(
                            "branch %d tag value = %q, want %q",
                            index as i64,
                            got,
                            tt.tag_values[index]
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/typeparser/returning_dispatch_test.go:253 TestParseTagMatch
#[test]
fn test_parse_tag_match() {
    run(|t| {
        t.parallel();
        let tests = [
            ("strict equality", r#"error._tag === "NotFound""#, "error", r#""NotFound""#),
            (
                "reversed loose equality",
                r#"("Timeout" as string) == error._tag"#,
                "error",
                r#""Timeout""#,
            ),
            (
                "nested subject",
                r#"error.reason._tag === "Invalid""#,
                "error.reason",
                r#""Invalid""#,
            ),
            ("inequality", r#"error._tag !== "NotFound""#, "", ""),
            ("different property", r#"error.kind === "NotFound""#, "", ""),
            ("compound expression", r#"error._tag === "NotFound" && ready"#, "", ""),
        ];

        for (name, expression, want_subject, want_value) in tests {
            t.run(name, |t| {
                t.parallel();
                let (sf, function) = parse_returning_dispatch_test_function(
                    t,
                    &format!("(error: any) => {expression}"),
                );
                let mut nil_program = nil_type_parser_program(t);
                let (subject, value) =
                    nil_program.type_parser().parse_tag_match(get_function_like_body(function));
                let got = returning_dispatch_node_text(Some(sf), subject);
                if got != want_subject {
                    t.fatal(sprintf!("tag subject = %q, want %q", got, want_subject));
                }
                let got = returning_dispatch_node_text(Some(sf), value);
                if got != want_value {
                    t.fatal(sprintf!("tag value = %q, want %q", got, want_value));
                }
            });
        }
    });
}

// go test: internal/typeparser/returning_dispatch_test.go:284 TestResultDispatchCommonTagSubject
#[test]
fn test_result_dispatch_common_tag_subject() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r#"
		class ReasonA { readonly _tag = "ReasonA" }
		class ReasonB { readonly _tag = "ReasonB" }
		class Wrapper {
			constructor(
				readonly reason: ReasonA | ReasonB,
				readonly alternateReason: ReasonA | ReasonB
			) {}
		}
		declare const other: Wrapper

		const sameIdentifier = (reason: ReasonA | ReasonB) =>
			reason._tag === "ReasonA" ? 1 : reason._tag === "ReasonB" ? 2 : 0

		const samePropertyChain = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (error.reason._tag === "ReasonB") return 2
			return 0
		}

		const sameSwitchSubject = (error: Wrapper) => {
			switch (error.reason._tag) {
				case "ReasonA": return 1
				case "ReasonB": return 2
				default: return 0
			}
		}

		const differentRoot = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (other.reason._tag === "ReasonB") return 2
			return 0
		}

		const differentPropertyChain = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (error.alternateReason._tag === "ReasonB") return 2
			return 0
		}

		const partlyUntagged = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (Boolean(error.reason)) return 2
			return 0
		}
	"#,
        );
        let mut tp = compiled.type_parser();

        let tests = [
            ("sameIdentifier", "reason"),
            ("samePropertyChain", "error.reason"),
            ("sameSwitchSubject", "error.reason"),
            ("differentRoot", ""),
            ("differentPropertyChain", ""),
            ("partlyUntagged", ""),
        ];
        for (name, want_subject) in tests {
            let function = find_returning_dispatch_test_function(t, sf, name);
            let Some(parsed) = tp.parse_returning_dispatch(function) else {
                t.fatal(sprintf!("%s: expected returning dispatch", name));
            };
            let subject = ResultDispatch::common_tag_subject(Some(&parsed.dispatch), &mut tp);
            let got = returning_dispatch_node_text(Some(sf), subject);
            if got != want_subject {
                t.fatal(sprintf!("%s: common tag subject = %q, want %q", name, got, want_subject));
            }
        }
    });
}

// go test: internal/typeparser/returning_dispatch_test.go:358 TestParseReturningDispatchInputContract
// Rust: Go ranges over a map literal (random order) and stops at the first failure; the cases run in a fixed order.
// Go prints the dispatch with `%+v` (pointer addresses); its `Debug` form stands in.
#[test]
fn test_parse_returning_dispatch_input_contract() {
    run(|t| {
        t.parallel();
        let sf = parse_source(
            r"
		function declaration(value: unknown) {
			return isA(value) ? first : fallback
		}
		const bodyOnly = (value: unknown) => isA(value) ? first : fallback
		const plain = (value: unknown) => first
		const generic = <A>(value: A) => isA(value) ? first : fallback
	",
        );

        #[derive(Default)]
        struct Found {
            declaration: Option<P<Node>>,
            body: Option<P<Node>>,
            plain: Option<P<Node>>,
            generic: Option<P<Node>>,
        }
        fn walk(node: P<Node>, found: &mut Found) -> bool {
            match get_function_like_name(node).as_str() {
                "declaration" => found.declaration = Some(node),
                "bodyOnly" => found.body = get_function_like_body(node),
                "plain" => found.plain = Some(node),
                "generic" => found.generic = Some(node),
                _ => {}
            }
            node.for_each_child(&mut |child| walk(child, found))
        }
        let mut found = Found::default();
        walk(sf.as_node(), &mut found);

        let mut nil_program = nil_type_parser_program(t);
        let mut tp = nil_program.type_parser();
        for (name, node) in [
            ("function declaration", found.declaration),
            ("body node", found.body),
            ("plain function", found.plain),
            ("generic function", found.generic),
        ] {
            let Some(node) = node else {
                t.fatal(sprintf!("%s fixture was not found", name));
            };
            if let Some(dispatch) = tp.parse_returning_dispatch(node) {
                t.fatal(format!("{name}: expected parse rejection, got {dispatch:?}"));
            }
        }
    });
}

// go test: internal/typeparser/returning_dispatch_test.go:407 findReturningDispatchTestFunction
pub(crate) fn find_returning_dispatch_test_function(
    t: &T,
    sf: P<SourceFile>,
    name: &str,
) -> P<Node> {
    t.helper();
    fn walk(node: P<Node>, name: &str, function: &mut Option<P<Node>>) -> bool {
        if (node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression)
            && get_function_like_name(node) == name
        {
            *function = Some(node);
            return true;
        }
        node.for_each_child(&mut |child| walk(child, name, function))
    }
    let mut function = None;
    walk(sf.as_node(), name, &mut function);
    let Some(function) = function else {
        t.fatal(sprintf!("function %s not found", name));
    };
    function
}

// go test: internal/typeparser/returning_dispatch_test.go:428 parseReturningDispatchTestFunction
fn parse_returning_dispatch_test_function(
    t: &T,
    function_source: &str,
) -> (P<SourceFile>, P<Node>) {
    t.helper();
    let sf = parse_source(&format!("const decode = {function_source}"));
    fn walk(node: P<Node>, function: &mut Option<P<Node>>) -> bool {
        if node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression {
            *function = Some(node);
            return true;
        }
        node.for_each_child(&mut |child| walk(child, function))
    }
    let mut function = None;
    walk(sf.as_node(), &mut function);
    let Some(function) = function else {
        t.fatal("failed to parse returning function");
    };
    (sf, function)
}

// go test: internal/typeparser/returning_dispatch_test.go:450 returningDispatchNodeText
pub(crate) fn returning_dispatch_node_text(
    sf: Option<P<SourceFile>>,
    node: Option<P<Node>>,
) -> String {
    let (Some(sf), Some(node)) = (sf, node) else {
        return String::new();
    };
    ets_shim::scanner::get_source_text_of_node_from_source_file(sf, node, false).trim().to_string()
}
