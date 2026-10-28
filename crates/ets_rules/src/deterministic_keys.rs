use std::sync::LazyLock;

use ets_etscore::{KeyPattern, ResolvedEffectPluginOptions, Severity};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, ClassLikeBase, Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::Program;
use ets_shim::core::TextRange;
use ets_shim::core::stringutil::go_strings_to_lower;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_KEY_DOES_NOT_MATCH_THE_DETERMINISTIC_KEY_FOR_THIS_DECLARATION_THE_EXPECTED_KEY_IS_0_EFFECT_DETERMINISTIC_KEYS;

// go: internal/rules/deterministic_keys.go:20 DeterministicKeys ec77d85411da6388
// DeterministicKeys ensures string key literals in Effect service/tag/error constructors
// follow a deterministic, location-based naming convention.
pub static DETERMINISTIC_KEYS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "deterministicKeys",
    group: "style",
    description: "Enforces deterministic naming for service/tag/error identifiers based on class names",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let (program, source_file, options) = (ctx.program, ctx.source_file, ctx.options);
        let matches = analyze_deterministic_keys(ctx.type_parser, program, source_file, options);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.expected_key],
            ));
        }
        diags
    },
});

// go: internal/rules/deterministic_keys.go:39 DeterministicKeyMatch d9c5442427ade459
// DeterministicKeyMatch holds the AST nodes and computed key info needed by both
// the diagnostic rule and the quick-fix for the deterministicKeys pattern.
#[derive(Clone)]
pub struct DeterministicKeyMatchExported {
    pub source_file: P<SourceFile>,  // The source file of the match
    pub location: TextRange,         // The pre-computed error range for the key string literal
    pub key_string_literal: P<Node>, // The key string literal node
    pub actual_key: String,          // The actual key string found in the source
    pub expected_key: String,        // The expected key computed by keybuilder
}

// go: internal/rules/deterministic_keys.go:49 AnalyzeDeterministicKeys 294c50e3850a8a78
// AnalyzeDeterministicKeys finds all class declarations where the key string literal
// doesn't match the expected deterministic key.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_deterministic_keys(
    tp: &mut TypeParser<'_>,
    program: &'static dyn Program,
    sf: P<SourceFile>,
    effect_config: Option<&ResolvedEffectPluginOptions>,
) -> Vec<DeterministicKeyMatchExported> {
    let Some(effect_config) = effect_config else {
        return Vec::new();
    };

    let key_patterns = ResolvedEffectPluginOptions::get_key_patterns(Some(effect_config));
    let extended_key_detection = effect_config.extended_key_detection;

    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::ClassDeclaration && node.name().is_some() {
            if let Some(m) = check_deterministic_key_match(
                tp,
                program,
                sf,
                node,
                key_patterns,
                extended_key_detection,
            ) {
                matches.push(m);
            }
        }

        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/deterministic_keys.go:83 deterministicKeyMatch 74cf8ad545c5f768
// deterministicKeyMatch holds the matched key info from a class declaration.
//
// Rust: Go's `deterministicKeyMatch` and `DeterministicKeyMatch` clash after capitalization; the exported one is
// `DeterministicKeyMatchExported` (docs/PORTING.md, "Naming").
pub(crate) struct DeterministicKeyMatch {
    pub(crate) class_name: Option<P<Node>>,
    pub(crate) key_string_literal: Option<P<Node>>,
    pub(crate) target: &'static str,
}

// go: internal/rules/deterministic_keys.go:89 checkDeterministicKeyMatch 388031c463219885
pub(crate) fn check_deterministic_key_match(
    tp: &mut TypeParser<'_>,
    program: &'static dyn Program,
    sf: P<SourceFile>,
    class_node: P<Node>,
    key_patterns: &[KeyPattern],
    extended_key_detection: bool,
) -> Option<DeterministicKeyMatchExported> {
    let m = match_class_pattern(tp, sf, class_node, extended_key_detection)?;
    let (Some(key_string_literal), Some(class_name)) = (m.key_string_literal, m.class_name) else {
        return None;
    };

    let expected_key = ets_keybuilder::create_string_for_source_file(
        program,
        tp,
        sf,
        &ets_shim::scanner::get_text_of_node(class_name),
        m.target,
        key_patterns,
    );
    if expected_key.is_empty() {
        return None;
    }

    // Get actual key
    let actual_key = key_string_literal.text().to_string();

    if actual_key == expected_key {
        return None;
    }

    Some(DeterministicKeyMatchExported {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, key_string_literal),
        key_string_literal,
        actual_key,
        expected_key,
    })
}

// go: internal/rules/deterministic_keys.go:118 matchClassPattern 701ccd86457afe9a
// matchClassPattern tries to match a class declaration against the supported patterns.
// Priority: service targets first, then error targets, then tagged class targets, then custom.
pub(crate) fn match_class_pattern(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    class_node: P<Node>,
    extended_key_detection: bool,
) -> Option<DeterministicKeyMatch> {
    // Service target: ExtendsEffectService → ExtendsContextTag → ExtendsEffectTag → ExtendsServiceMapService
    if let Some(result) = tp.extends_effect_v3_service(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.key_string_literal(),
            target: "service",
        });
    }
    if let Some(result) = tp.extends_context_tag(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.key_string_literal(),
            target: "service",
        });
    }
    if let Some(result) = tp.extends_effect_tag(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.key_string_literal(),
            target: "service",
        });
    }
    if let Some(result) = tp.extends_context_service(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.key_string_literal(),
            target: "service",
        });
    }

    // Error target: ExtendsDataTaggedError → ExtendsSchemaTaggedError
    if let Some(result) = tp.extends_data_tagged_error(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: result.class_name(),
            key_string_literal: result.key_string_literal(),
            target: "error",
        });
    }
    if let Some(result) = tp.extends_schema_tagged_error(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.key_string_literal(),
            target: "error",
        });
    }

    // Tagged class target: ExtendsSchemaTaggedClass → ExtendsDataTaggedClass → ExtendsSchemaTaggedRequest
    // For Schema.* the checked literal is the tag, not the optional inner identifier.
    if let Some(result) = tp.extends_schema_tagged_class(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.tag_string_literal(),
            target: "taggedClass",
        });
    }
    if let Some(result) = tp.extends_data_tagged_class(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: result.class_name(),
            key_string_literal: result.key_string_literal(),
            target: "taggedClass",
        });
    }
    if let Some(result) = tp.extends_schema_tagged_request(class_node) {
        return Some(DeterministicKeyMatch {
            class_name: Some(result.class_name()),
            key_string_literal: result.tag_string_literal(),
            target: "taggedClass",
        });
    }

    // Custom target (only if extendedKeyDetection is enabled)
    if extended_key_detection {
        if let Some(result) = match_custom_pattern(tp, sf, class_node) {
            return Some(result);
        }
    }

    None
}

// go: internal/rules/deterministic_keys.go:165 matchCustomPattern 39bd05fe97d24b59
// matchCustomPattern checks heritage clause nodes for call expressions with string literal
// arguments whose parameter declarations contain the @effect-identifier annotation.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); the source file is unused, as in Go.
pub(crate) fn match_custom_pattern(
    tp: &mut TypeParser<'_>,
    _sf: P<SourceFile>,
    class_node: P<Node>,
) -> Option<DeterministicKeyMatch> {
    let class_name = class_node.name()?;

    let heritage_clauses =
        class_node.class_like_data().and_then(ClassLikeBase::heritage_clauses)?;

    // BFS through heritage clause nodes
    let mut nodes_to_visit: std::collections::VecDeque<P<Node>> =
        heritage_clauses.nodes().iter().copied().collect();

    while let Some(current) = nodes_to_visit.pop_front() {
        if ast::is_call_expression(current) {
            let call = current.as_call_expression();
            for (i, &arg) in call.arguments().nodes().iter().enumerate() {
                if !ast::is_string_literal(arg) {
                    continue;
                }

                let sig = tp.checker().get_resolved_signature_exported(current);

                let params = sig.parameters();
                if i >= params.len() {
                    continue;
                }

                let param = params[i];
                if param.declarations().is_empty() {
                    continue;
                }

                for &decl in param.declarations() {
                    let Some(param_sf) = ast::get_source_file_of_node(decl) else {
                        continue;
                    };
                    let param_text = param_sf.text();
                    let (Some(pos), Some(end)) =
                        (decl.pos().source_offset(), decl.end().source_offset())
                    else {
                        continue;
                    };
                    let (pos, end) = (pos as usize, end as usize);
                    if end >= pos && end <= param_text.len() {
                        let decl_text = &param_text[pos..end];
                        if go_strings_to_lower(decl_text).contains("@effect-identifier") {
                            return Some(DeterministicKeyMatch {
                                class_name: Some(class_name),
                                key_string_literal: Some(arg),
                                target: "custom",
                            });
                        }
                    }
                }
            }
        }

        // Visit children
        current.for_each_child(&mut |child| {
            nodes_to_visit.push_back(child);
            false
        });
    }

    None
}

// go: internal/rules/deterministic_keys.go:241 getPackageJsonDirectory 91834641597a83b0
// getPackageJsonDirectory gets the package.json directory for a source file from its metadata.
//
// Rust: Go's checker argument is unused and dropped. Go's assertion to its `metaProvider` interface always
// succeeds (`GetSourceFileMetaData` is a `checker.Program` method). `pub` rather than `pub(crate)` until its
// callers (apiStabilityLeak, the stability API usage rules) are ported, so it is not dead code meanwhile.
pub fn get_package_json_directory(program: &'static dyn Program, sf: P<SourceFile>) -> String {
    let meta = program.get_source_file_meta_data(sf.path());
    meta.package_json_directory
}
