// Package etslshooks provides Effect code fix integration with TypeScript-Go.
// This package registers a single CodeFixProvider that delegates to internal/fixables.
//
// Import this package with a blank import in cmd/tsgo/main.go to register
// Effect code fix providers:
//
//	import _ "github.com/effect-ts/tsgo/etslshooks"

use std::fmt::Write as _;
use std::sync::{Arc, LazyLock};

use ets_etscore::{EffectPluginOptions, ResolvedEffectPluginOptions};
use ets_layergraph::ExtractLayerGraphOptions;
use ets_shim::ast::{Kind, Node, SourceFile, Symbol};
use ets_shim::checker::types::TypeFormatFlags;
use ets_shim::checker::{Checker, Type};
use ets_shim::compiler::Program;
use ets_shim::core::P;
use ets_shim::core::TextRange;
use ets_shim::core::context::Context;
use ets_shim::ls::autoimport::{self, Export, FixTransformer};
use ets_shim::ls::{self, CodeAction, LanguageService};
use ets_shim::lsproto;
use ets_shim::modulespecifiers::{self, ModuleSpecifierOptions, ResultKind};
use ets_typeparser::{Effect, EffectContextFlags, TypeParser};

use crate::document_symbols::after_document_symbols;
use crate::inlay_hints::after_inlay_hints;

// go: etslshooks/init.go:37 init
pub(crate) fn init() {
    // Register the Effect code fix provider with the language service
    ls::register_code_fix_provider(&EFFECT_FIX_PROVIDER);
    // Register the Effect refactor provider with the language service
    ls::register_refactor_provider(&EFFECT_REFACTOR_PROVIDER);
    // Register the Effect hover enrichment callback
    ls::register_after_quick_info_callback(after_quick_info);
    // Register the Effect document symbol enrichment callback
    ls::register_after_document_symbols_callback(after_document_symbols);
    // Register the Effect inlay hints suppression callback
    ls::register_after_inlay_hints_callback(after_inlay_hints);
    // Register the Effect completion enrichment callback
    ls::register_after_completion_callback(after_completion);
    // Register the Effect auto-import style transformer factory
    autoimport::register_auto_import_fix_transformer(Arc::new(
        |preferences: &modulespecifiers::UserPreferences,
         program: &'static Program,
         importing_file: P<SourceFile>|
         -> Option<FixTransformer> {
            let mut resolved_options: Option<ResolvedEffectPluginOptions> = None;
            if let Some(effect_config) = program.options().effect.get() {
                resolved_options = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
                    Some(&effect_config),
                    importing_file.file_name(),
                    &program.options().config_file_path,
                    program.use_case_sensitive_file_names(),
                );
            }
            let preferences = preferences.clone();
            ets_autoimportstyle::new_fix_transformer(
                resolved_options.as_ref(),
                Some(program),
                Some(Box::new(move |export: &Export| -> (String, ResultKind) {
                    // Go's shim `autoimport.ModuleIDFileName`: the path of a file module, empty for an ambient one.
                    let mut target_file_name =
                        export.target.module_id.as_file().unwrap_or_default().to_string();
                    if let Some(target_file) = program.get_source_file(&target_file_name) {
                        target_file_name = target_file.file_name().to_string();
                    }
                    let (specifiers, kind) =
                        modulespecifiers::get_module_specifiers_for_file_with_info(
                            importing_file,
                            &target_file_name,
                            &program.options(),
                            program,
                            &preferences,
                            ModuleSpecifierOptions::default(),
                            true,
                        );
                    for specifier in specifiers {
                        if !specifier.contains("/node_modules/") {
                            return (specifier, kind);
                        }
                    }
                    (String::new(), ResultKind::None)
                })),
            )
        },
    ));
}

// go: etslshooks/init.go:87 effectFixProvider
// effectFixProvider is the CodeFixProvider that handles all Effect diagnostic codes.
// It delegates to the fixables registered in internal/fixables.
//
// Rust: tsrs's `CodeFixProvider` holds `'static` slices; the computed code and fix-id lists are leaked once (the
// provider lives for the process, as Go's package variable does).
pub(crate) static EFFECT_FIX_PROVIDER: LazyLock<ls::CodeFixProvider> =
    LazyLock::new(|| ls::CodeFixProvider {
        error_codes: Box::leak(ets_fixables::all_error_codes().into_boxed_slice()),
        get_code_actions: get_effect_code_actions,
        fix_ids: Box::leak(ets_fixables::all_fix_ids().into_boxed_slice()),
        get_all_code_actions: None,
    });

// go: etslshooks/init.go:94 getEffectCodeActions
// getEffectCodeActions finds applicable fixables and collects their code actions.
//
// Rust: Go's `fixCtx.Program != nil` and `ch != nil` always hold here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "tsrs's provider signature (patch 011) is fallible, as Go's (`[]*ls.CodeAction, error`)"
)]
fn get_effect_code_actions(
    ctx: &Context,
    fix_ctx: &ls::CodeFixContext<'_>,
) -> Result<Vec<CodeAction>, lsproto::Error> {
    let program = fix_ctx.program;
    if let Some(parsed_effect_config) = program.options().effect.get() {
        let options = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
            Some(&parsed_effect_config),
            fix_ctx.source_file.file_name(),
            &program.options().config_file_path,
            program.use_case_sensitive_file_names(),
        );

        let mut ch = program.get_type_checker_for_file(ctx, fix_ctx.source_file);

        let tp = ets_typeparser::new_type_parser(program, &mut ch);
        let mut f_ctx = ets_fixable::new_context(ctx, fix_ctx, options.as_ref(), tp);
        let mut actions = Vec::new();
        for provider in ets_fixables::by_error_code(fix_ctx.error_code) {
            for result in (provider.run)(&mut f_ctx) {
                actions.push(result);
            }
        }
        return Ok(actions);
    }

    Ok(Vec::new())
}

// go: etslshooks/init.go:128 effectRefactorProvider
// effectRefactorProvider is the RefactorProvider that handles all Effect refactoring actions.
// It delegates to the refactors registered in internal/refactors.
pub(crate) static EFFECT_REFACTOR_PROVIDER: ls::RefactorProvider =
    ls::RefactorProvider { get_refactor_actions: get_effect_refactor_actions };

// go: etslshooks/init.go:133 getEffectRefactorActions
// getEffectRefactorActions iterates all registered refactors and collects their code actions.
#[expect(
    clippy::unnecessary_wraps,
    reason = "tsrs's provider signature (patch 011) is fallible, as Go's (`[]ls.CodeAction, error`)"
)]
fn get_effect_refactor_actions(
    ctx: &Context,
    file: P<SourceFile>,
    span: TextRange,
    program: &'static Program,
    lang_service: &LanguageService,
) -> Result<Vec<CodeAction>, lsproto::Error> {
    let effect_config = program.options().effect.get();
    if effect_config.is_none()
        || !EffectPluginOptions::get_refactors_enabled(effect_config.as_deref())
    {
        return Ok(Vec::new());
    }

    let mut ch = program.get_type_checker_for_file(ctx, file);
    let tp = ets_typeparser::new_type_parser(program, &mut ch);

    let mut r_ctx = ets_refactor::new_context(ctx, file, span, program, lang_service, tp);

    let mut actions = Vec::new();
    for r in ets_refactors::ALL.iter() {
        let results = (r.run)(&mut r_ctx);
        actions.extend(results);
    }

    Ok(actions)
}

// go: etslshooks/init.go:155 afterCompletion
// afterCompletion is called after TypeScript-Go builds the completion list.
// It allows Effect to enrich completion responses with custom completions.
//
// Rust: the completion context borrows a copy of the incoming items (Go shares the slice header; its length does
// not change while the completions run).
fn after_completion(
    ctx: &Context,
    sf: P<SourceFile>,
    position: u32,
    mut items: Vec<lsproto::CompletionItem>,
    program: &'static Program,
    lang_service: &LanguageService,
) -> Vec<lsproto::CompletionItem> {
    let effect_config = program.options().effect.get();
    if effect_config.is_none()
        || !EffectPluginOptions::get_completions_enabled(effect_config.as_deref())
    {
        return items;
    }

    if ets_completions::ALL.is_empty() {
        return items;
    }

    let mut ch = program.get_type_checker_for_file(ctx, sf);
    let tp = ets_typeparser::new_type_parser(program, &mut ch);

    let existing_items = items.clone();
    let mut completion_ctx =
        ets_completion::new_context(ctx, sf, position, &existing_items, program, lang_service, tp);

    for c in ets_completions::ALL.iter() {
        let results = (c.run)(&mut completion_ctx);
        items.extend(results);
    }

    items
}

// go: etslshooks/init.go:181 afterQuickInfo
// afterQuickInfo is called after building hover quickInfo and documentation.
// It allows Effect to enrich hover responses with Effect-specific information.
fn after_quick_info(
    program: &'static Program,
    c: &mut Checker,
    sf: P<SourceFile>,
    node: P<Node>,
    _symbol: Option<P<Symbol>>,
    quick_info: String,
    documentation: String,
    is_markdown: bool,
) -> (String, String, Option<P<Node>>) {
    let mut tp = ets_typeparser::new_type_parser(program, c);

    // Check if Effect is enabled
    let effect_config = program.options().effect.get();
    if effect_config.is_none()
        || !EffectPluginOptions::get_quickinfo_enabled(effect_config.as_deref())
    {
        return (quick_info, documentation, None);
    }
    let effect_config = effect_config.unwrap();

    // Yield* hover: normalize positions on yield, *, and the space before the operand.
    let mut yield_node: Option<P<Node>> = None;
    match node.kind() {
        Kind::YieldExpression => yield_node = Some(node),
        Kind::YieldKeyword | Kind::AsteriskToken => {
            if let Some(parent) = node.parent() {
                if parent.kind() == Kind::YieldExpression {
                    yield_node = Some(parent);
                }
            }
        }
        _ => {}
    }
    if let Some(yield_node) = yield_node {
        let yield_ = yield_node.as_yield_expression();
        if yield_.asterisk_token().is_some() {
            if let Some(expression) = yield_.expression() {
                if tp
                    .get_effect_context_flags(yield_node)
                    .intersects(EffectContextFlags::CanYieldEffect)
                {
                    let t = tp.get_type_at_location(expression);
                    if let Some(t) = t {
                        let effect = tp.effect_yieldable_type(t);
                        if let Some(effect) = effect {
                            let type_str = tp.checker().type_to_string_ex(
                                t,
                                None,
                                TypeFormatFlags::NoTruncation,
                                None,
                            );
                            let quick_info = "(yield*) ".to_string() + &type_str;
                            let documentation =
                                format_effect_type_params(tp.checker(), &effect, "", is_markdown);
                            return (quick_info, documentation, Some(yield_node));
                        }
                    }
                }
            }
        }
    }

    // General symbol hover: enrich Effect-typed symbols with type parameters
    let Some(t) = tp.get_type_at_location(node) else {
        return (quick_info, documentation, None);
    };

    // Layer hover: detect Layer types and show providers/requirers summary.
    // Layer extends Effect in V4, so this check must come before the Effect check.
    // Only activate layer hover enrichment when the cursor is on the name of the declaration,
    // not on arbitrary nodes within the initializer expression.
    if tp.is_layer_type(t) && is_declaration_name(node) {
        let documentation =
            format_layer_hover(&mut tp, sf, node, t, &documentation, is_markdown, &effect_config);
        return (quick_info, documentation, None);
    }

    let Some(effect) = tp.effect_type(t) else {
        return (quick_info, documentation, None);
    };

    let documentation =
        format_effect_type_params(tp.checker(), &effect, &documentation, is_markdown);

    (quick_info, documentation, None)
}

// go: etslshooks/init.go:245 formatLayerHover
// formatLayerHover builds the Layer hover documentation including providers/requirers
// summary, Mermaid diagram links, and Layer type parameters.
//
// Rust: Go's separate checker parameter is `tp`'s checker (`tp.checker()`).
fn format_layer_hover(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
    _t: P<Type>,
    documentation: &str,
    is_markdown: bool,
    effect_config: &EffectPluginOptions,
) -> String {
    // Try to resolve the initializer expression for layer graph extraction.
    let mut initializer: Option<P<Node>> = None;
    if let Some(parent) = node.parent() {
        match parent.kind() {
            Kind::VariableDeclaration => {
                initializer = parent.as_variable_declaration().initializer();
            }
            Kind::PropertyDeclaration => {
                initializer = parent.as_property_declaration().initializer();
            }
            _ => {}
        }
    }

    let mut quick_info_summary = String::new();
    let mut has_graph = false;
    let mut nested_diagram = String::new();
    let mut outline_diagram = String::new();
    if let Some(initializer) = initializer {
        let opts = ExtractLayerGraphOptions {
            follow_symbols_depth: EffectPluginOptions::get_layer_graph_follow_depth(Some(
                effect_config,
            )),
            ..Default::default()
        };
        let full_graph = ets_layergraph::extract_layer_graph(tp, &[initializer], Some(sf), opts);
        let info = ets_layergraph::extract_providers_and_requirers(tp.checker(), &full_graph);
        quick_info_summary = ets_layergraph::format_quick_info(tp.checker(), &info, Some(sf));
        has_graph = true;

        if !effect_config.no_external {
            nested_diagram =
                ets_layergraph::format_nested_layer_graph(tp.checker(), &full_graph, Some(sf));
            let outline_graph = ets_layergraph::extract_outline_graph(tp, &full_graph);
            outline_diagram =
                ets_layergraph::format_outline_graph(tp.checker(), &outline_graph, Some(sf));
        }
    }

    // Build combined documentation: quickinfo summary (provides/requires) and links.
    let mut b = String::new();

    if !quick_info_summary.is_empty() {
        if is_markdown {
            b.push_str("```\n");
            b.push_str(&quick_info_summary);
            b.push_str("\n```\n");
        } else {
            b.push_str(&quick_info_summary);
            b.push('\n');
        }
    }

    // Generate Mermaid diagram links when we have a graph and external links are not suppressed.
    if has_graph && !effect_config.no_external {
        let base_url = EffectPluginOptions::get_mermaid_base_url(Some(effect_config));

        let mut nested_url = String::new();
        let mut outline_url = String::new();
        if !nested_diagram.is_empty() {
            nested_url = ets_layergraph::encode_mermaid_url(&base_url, &nested_diagram);
        }
        if !outline_diagram.is_empty() {
            outline_url = ets_layergraph::encode_mermaid_url(&base_url, &outline_diagram);
        }

        if is_markdown {
            if !nested_url.is_empty() && !outline_url.is_empty() {
                let _ = write!(
                    b,
                    "[Show full graph]({nested_url}) - [Show outline]({outline_url})\n\n"
                );
            } else if !nested_url.is_empty() {
                let _ = write!(b, "[Show full graph]({nested_url})\n\n");
            } else if !outline_url.is_empty() {
                let _ = write!(b, "[Show outline]({outline_url})\n\n");
            }
        } else {
            if !nested_url.is_empty() {
                let _ = write!(b, "{{@link {nested_url} Show full graph}}\n\n");
            }
            if !outline_url.is_empty() {
                let _ = write!(b, "{{@link {outline_url} Show outline}}\n\n");
            }
        }
    }

    if !documentation.is_empty() {
        b.push('\n');
        b.push_str(documentation);
    }

    b
}

// go: etslshooks/init.go:332 isDeclarationName
// isDeclarationName checks whether the given node is the name node of a variable or property declaration.
// This is used to restrict layer hover enrichment to the declaration name only,
// not to arbitrary nodes within the initializer expression.
fn is_declaration_name(node: P<Node>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        Kind::VariableDeclaration | Kind::PropertyDeclaration => parent.name() == Some(node),
        _ => false,
    }
}

// go: etslshooks/init.go:346 formatEffectTypeParams
// formatEffectTypeParams formats Effect type parameters (A, E, R) and prepends them to documentation.
fn format_effect_type_params(
    c: &mut Checker,
    effect: &Effect,
    documentation: &str,
    is_markdown: bool,
) -> String {
    let a_str = c.type_to_string_ex(effect.a(), None, TypeFormatFlags::NoTruncation, None);
    let e_str = c.type_to_string_ex(effect.e(), None, TypeFormatFlags::NoTruncation, None);
    let r_str = c.type_to_string_ex(effect.r(), None, TypeFormatFlags::NoTruncation, None);

    let prefix = if is_markdown {
        format!(
            "```ts\n/* Effect Type Parameters */\ntype Success = {a_str}\ntype Failure = {e_str}\ntype Requirements = {r_str}\n```\n"
        )
    } else {
        format!(
            "Effect Type Parameters:\n  Success = {a_str}\n  Failure = {e_str}\n  Requirements = {r_str}\n"
        )
    };

    let mut b = String::new();
    b.push_str(&prefix);
    if !documentation.is_empty() {
        b.push('\n');
        b.push_str(documentation);
    }
    b
}
