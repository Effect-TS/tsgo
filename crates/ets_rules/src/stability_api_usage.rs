use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::{Checker, Signature, Type};
use ets_shim::core::tspath;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::deterministic_keys::get_package_json_directory;

/// Rust-only: the rules' messages (Go spells them out at each use).
static EXPERIMENTAL_MESSAGE: &Message =
    &tsdiag::X_0_IS_AN_EXPERIMENTAL_API_EFFECT_EXPERIMENTAL_API_USAGE;
/// Rust-only: see `EXPERIMENTAL_MESSAGE`.
static UNSTABLE_MESSAGE: &Message = &tsdiag::X_0_IS_AN_UNSTABLE_API_BREAKING_CHANGES_MAY_HAPPEN_BETWEEN_VERSIONS_EFFECT_UNSTABLE_API_USAGE;

// go: internal/rules/stability_api_usage.go:16 ExperimentalApiUsage
pub static EXPERIMENTAL_API_USAGE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "experimentalApiUsage",
    group: "correctness",
    description: "Warns when using an API marked @stability experimental",
    default_severity: Severity::Warning,
    supported_effect: vec!["v4"],
    codes: vec![EXPERIMENTAL_MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        run_stability_api_usage(ctx, "experimental")
    },
});

// go: internal/rules/stability_api_usage.go:28 UnstableApiUsage
pub static UNSTABLE_API_USAGE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unstableApiUsage",
    group: "correctness",
    description: "Warns when using an API marked @stability unstable",
    default_severity: Severity::Warning,
    supported_effect: vec!["v4"],
    codes: vec![UNSTABLE_MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        run_stability_api_usage(ctx, "unstable")
    },
});

// go: internal/rules/stability_api_usage.go:40 declarationStabilityInfo
#[derive(Clone, Copy, Default)]
pub(crate) struct DeclarationStabilityInfo {
    pub(crate) stability: &'static str,
    pub(crate) declaration: Option<P<Node>>,
}

// go: internal/rules/stability_api_usage.go:45 runStabilityApiUsage
//
// Rust: Go's closures (`readSymbol`, `readSignature`, `report`, `walk`) are nested functions that take what they
// capture (the rule context and a `StabilityApiUsageState` for `wanted`, the allow-list and the diagnostics).
pub(crate) fn run_stability_api_usage(
    ctx: &mut Context<'_, '_>,
    wanted: &'static str,
) -> Vec<P<Diagnostic>> {
    // Declared stability is cached per checker in TypeParser, so references to the
    // same symbol or selected overload reuse one lookup across source files. The
    // selected overload's own tag always wins over the symbol-level tag.
    fn read_symbol(
        ctx: &mut Context<'_, '_>,
        symbol: Option<P<Symbol>>,
    ) -> DeclarationStabilityInfo {
        let Some(symbol) = symbol else {
            return DeclarationStabilityInfo::default();
        };
        let info = ctx.type_parser.declared_api_stability_of_symbol(symbol);
        DeclarationStabilityInfo {
            stability: ets_typeparser::api_stability_level_tag(info.level()),
            declaration: info.declaration(),
        }
    }
    fn read_signature(
        ctx: &mut Context<'_, '_>,
        signature: Option<P<Signature>>,
    ) -> DeclarationStabilityInfo {
        let Some(signature) = signature else {
            return DeclarationStabilityInfo::default();
        };
        let info = ctx.type_parser.declared_api_stability_of_signature(signature);
        DeclarationStabilityInfo {
            stability: ets_typeparser::api_stability_level_tag(info.level()),
            declaration: info.declaration(),
        }
    }

    struct State {
        wanted: &'static str,
        allow: StabilityApiAllowlist,
        diagnostics: Vec<P<Diagnostic>>,
    }

    fn report(
        ctx: &mut Context<'_, '_>,
        state: &mut State,
        node: P<Node>,
        name: &str,
        stability: DeclarationStabilityInfo,
    ) -> bool {
        if stability.stability != state.wanted {
            return false;
        }
        let (allowed, api_name) = state.allow.call(ctx, stability.declaration);
        if allowed {
            return false;
        }
        let name = if !api_name.is_empty() { api_name.as_str() } else { name };
        let mut message: &'static Message = UNSTABLE_MESSAGE;
        if state.wanted == "experimental" {
            message = EXPERIMENTAL_MESSAGE;
        }
        let diagnostic =
            ctx.new_diagnostic(ctx.source_file, ctx.get_error_range(node), message, &[], &[name]);
        state.diagnostics.push(diagnostic);
        true
    }

    fn walk(ctx: &mut Context<'_, '_>, state: &mut State, node: P<Node>) -> bool {
        // Object literal keys declare local properties, but also use the matching
        // properties of their contextual type. Resolve those declarations before
        // the ordinary reference path excludes declaration names.
        if ast::is_object_literal_element(node)
            && let Some(node_name) = node.name()
            && let Some(parent) = node.parent()
            && parent.kind() == Kind::ObjectLiteralExpression
        {
            let name = ast::get_text_of_property_name(node_name);
            if !name.is_empty()
                && let Some(contextual_type) =
                    ctx.type_parser.cached_object_literal_contextual_type(parent)
            {
                // Keep the original symbols and let the checker filter union
                // branches by discriminants before reading their own tags.
                let mut properties = ctx.checker().get_property_symbols_from_contextual_type(
                    node,
                    contextual_type,
                    false,
                );
                // Generic inference can point back to the literal's own
                // property. Use the type the literal gets without its own
                // inferences, as go-to-definition does, and the bare
                // constraints when that type has no other property, so its
                // stability tag is not lost to inference. Neither resolves the
                // call again.
                let is_own = |symbol: &P<Symbol>| symbol.value_declaration() == Some(node);
                if properties.iter().any(is_own) {
                    let lookups: [fn(&mut TypeParser<'_>, P<Node>) -> Option<P<Type>>; 2] = [
                        |tp, n| tp.cached_object_literal_constraint_contextual_type(n),
                        |tp, n| tp.cached_object_literal_bare_constraint_contextual_type(n),
                    ];
                    for lookup in lookups {
                        if let Some(constraint_type) = lookup(ctx.type_parser, parent) {
                            let constraint_properties =
                                ctx.checker().get_property_symbols_from_contextual_type(
                                    node,
                                    constraint_type,
                                    false,
                                );
                            if constraint_properties.iter().any(|symbol| !is_own(symbol)) {
                                properties = constraint_properties;
                                break;
                            }
                        }
                    }
                }
                for symbol in properties {
                    if symbol.value_declaration() == Some(node) {
                        continue;
                    }
                    let stability = read_symbol(ctx, Some(symbol));
                    if report(ctx, state, node_name, &name, stability) {
                        break;
                    }
                }
            }
        }
        if node.kind() == Kind::Identifier
            && !ast::is_declaration_name_or_import_property_name(node)
        {
            // The selected overload is authoritative for calls. A tagged overload
            // may differ from other declarations of the same symbol.
            let mut stability = DeclarationStabilityInfo::default();
            let mut selected_declaration: Option<P<Node>> = None;
            let mut callee = node;
            if let Some(parent) = node.parent()
                && parent.kind() == Kind::PropertyAccessExpression
                && parent.as_property_access_expression().name() == node
            {
                callee = parent;
            }
            if let Some(parent) = callee.parent()
                && (parent.kind() == Kind::CallExpression || parent.kind() == Kind::NewExpression)
                && parent.expression() == Some(callee)
            {
                let signature = ctx.checker().get_resolved_signature_exported(parent);
                if let Some(declaration) = signature.declaration() {
                    selected_declaration = Some(declaration);
                    stability = read_signature(ctx, Some(signature));
                    if stability.stability.is_empty() {
                        // A signature with no own tag still carries the selected
                        // declaration so the symbol fallback can be suppressed for it.
                        stability = DeclarationStabilityInfo {
                            stability: "",
                            declaration: selected_declaration,
                        };
                    }
                }
            }
            let (symbol, resolved_symbol) =
                ctx.type_parser.symbol_and_reference_symbol_at_node(node);
            let use_symbol = selected_declaration.is_none()
                || !symbol_has_declaration(symbol, selected_declaration)
                    && !symbol_has_declaration(resolved_symbol, selected_declaration);
            if stability.stability.is_empty() && use_symbol {
                stability = read_symbol(ctx, symbol);
            }
            if stability.stability.is_empty() && use_symbol {
                stability = read_symbol(ctx, resolved_symbol);
            }
            report(ctx, state, node, node.text(), stability);
        }
        node.for_each_child(&mut |child| walk(ctx, state, child));
        false
    }

    let allow = new_stability_api_allowlist(ctx, wanted);
    let mut state = State { wanted, allow, diagnostics: Vec::new() };
    let root = ctx.source_file.as_node();
    walk(ctx, &mut state, root);
    state.diagnostics
}

// go: internal/rules/stability_api_usage.go:167 symbolHasDeclaration
pub(crate) fn symbol_has_declaration(
    symbol: Option<P<Symbol>>,
    declaration: Option<P<Node>>,
) -> bool {
    let (Some(symbol), Some(declaration)) = (symbol, declaration) else {
        return false;
    };
    symbol.declarations().contains(&declaration)
}

// go: internal/rules/stability_api_usage.go:176 stabilitySymbolIsModuleExport
// stabilitySymbolIsModuleExport reports whether symbol is exported from the
// module under its own name.
pub(crate) fn stability_symbol_is_module_export(
    c: &mut Checker,
    symbol: Option<P<Symbol>>,
    module_symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(symbol), Some(module_symbol)) = (symbol, module_symbol) else {
        return false;
    };
    c.try_get_member_in_module_exports_and_properties(symbol.name(), module_symbol) == Some(symbol)
}

// go: internal/rules/stability_api_usage.go:188 stabilityOwningExportSymbol
// stabilityOwningExportSymbol walks out from a selected declaration to the
// nearest enclosing declaration that is exported from the module. A call or
// construct signature of a dual export has its own `__call` symbol, so the
// export it belongs to can only be found by climbing to the annotated
// declaration (for example the variable the object type is assigned to).
pub(crate) fn stability_owning_export_symbol(
    c: &mut Checker,
    declaration: P<Node>,
    module_symbol: Option<P<Symbol>>,
) -> Option<P<Symbol>> {
    let mut current = declaration.parent();
    while let Some(node) = current {
        current = node.parent();
        let Some(mut symbol) = ets_typeparser::get_symbol_of_declaration(c, node) else {
            continue;
        };
        if let Some(export_symbol) = symbol.export_symbol() {
            symbol = export_symbol;
        }
        if stability_symbol_is_module_export(c, Some(symbol), module_symbol) {
            return Some(symbol);
        }
    }
    None
}

/// Rust-only: Go's `decision` type, local to `newStabilityApiAllowlist`.
#[derive(Clone, Default)]
pub(crate) struct StabilityApiDecision {
    allowed: bool,
    name: String,
}

/// Rust-only: the state the closure returned by Go's `newStabilityApiAllowlist` captures. Calling the closure is
/// `call`, which takes the rule context (the context holds the parser, which cannot be stored; docs/PORTING.md,
/// "Types").
pub(crate) struct StabilityApiAllowlist {
    decisions: FxHashMap<Option<P<Node>>, StabilityApiDecision>,
    modules: FxHashMap<P<SourceFile>, String>,
    entries: Vec<String>,
}

// go: internal/rules/stability_api_usage.go:206 newStabilityApiAllowlist
// Decisions belong to one rule invocation: per-file overrides can change the
// allow-list, and the tagged declaration distinguishes overloads and reexports.
pub(crate) fn new_stability_api_allowlist(
    ctx: &Context<'_, '_>,
    wanted: &str,
) -> StabilityApiAllowlist {
    let decisions = FxHashMap::default();
    let modules = FxHashMap::default();
    let mut entries = Vec::new();
    if let Some(options) = ctx.options {
        entries.clone_from(&options.allowed_unstable_apis);
        if wanted == "experimental" {
            entries.clone_from(&options.allowed_experimental_apis);
        }
    }
    StabilityApiAllowlist { decisions, modules, entries }
}

impl StabilityApiAllowlist {
    // go: internal/rules/stability_api_usage.go:206 newStabilityApiAllowlist
    // Rust: the body of the closure Go returns.
    pub(crate) fn call(
        &mut self,
        ctx: &mut Context<'_, '_>,
        declaration: Option<P<Node>>,
    ) -> (bool, String) {
        if let Some(cached) = self.decisions.get(&declaration) {
            return (cached.allowed, cached.name.clone());
        }
        // Rust: Go's `defer func() { decisions[declaration] = result }()` stores the result on every return.
        let result = self.decide(ctx, declaration);
        self.decisions.insert(declaration, result.clone());
        (result.allowed, result.name)
    }

    // go: internal/rules/stability_api_usage.go:206 newStabilityApiAllowlist
    // Rust: the closure's body after the cache lookup; `call` stores what it returns, as Go's deferred store.
    fn decide(
        &mut self,
        ctx: &mut Context<'_, '_>,
        declaration: Option<P<Node>>,
    ) -> StabilityApiDecision {
        let mut result = StabilityApiDecision::default();
        let Some(declaration) = declaration else {
            return result;
        };
        let Some(sf) = ast::get_source_file_of_node(declaration) else {
            return result;
        };
        let module_name = if let Some(module_name) = self.modules.get(&sf) {
            module_name.clone()
        } else {
            let mut module_name = String::new();
            let pkg = ctx.type_parser.package_json_for_source_file(sf);
            if let Some(pkg) = pkg
                && let Some(package_name) = pkg.name.get_value()
                && !package_name.is_empty()
            {
                let directory = get_package_json_directory(ctx.program, sf);
                module_name = stability_api_module_name(package_name, &directory, sf.file_name());
            }
            self.modules.insert(sf, module_name.clone());
            module_name
        };
        if module_name.is_empty() {
            return result;
        }
        let module_symbol = ets_typeparser::get_symbol_of_declaration(ctx.checker(), sf.as_node());
        let mut symbol = ets_typeparser::get_symbol_of_declaration(ctx.checker(), declaration);
        if let Some(s) = symbol
            && let Some(export_symbol) = s.export_symbol()
        {
            symbol = Some(export_symbol);
        }
        // A selected overload (for example a call signature of a dual export)
        // carries its own `__call` symbol rather than the export's, so it is not
        // a module export. Resolve the enclosing exported declaration instead so
        // the name and the per-export allow-list match stay export-scoped.
        if module_symbol.is_some()
            && !stability_symbol_is_module_export(ctx.checker(), symbol, module_symbol)
            && let Some(owner) =
                stability_owning_export_symbol(ctx.checker(), declaration, module_symbol)
        {
            symbol = Some(owner);
        }
        result.name.clone_from(&module_name);
        if let (Some(module_symbol), Some(symbol)) = (module_symbol, symbol) {
            let exported = ctx
                .checker()
                .try_get_member_in_module_exports_and_properties(symbol.name(), module_symbol);
            if exported == Some(symbol) {
                result.name += "#";
                result.name += symbol.name();
            }
        }
        for entry in &self.entries {
            let (module, member, has_member) = match entry.split_once('#') {
                Some((module, member)) => (module, member, true),
                None => (entry.as_str(), "", false),
            };
            if !has_member {
                if !module.is_empty()
                    && (module_name == module || module_name.starts_with(&format!("{module}/")))
                {
                    result.allowed = true;
                    break;
                }
            } else if module == module_name
                && !member.is_empty()
                && let (Some(module_symbol), Some(symbol)) = (module_symbol, symbol)
            {
                let c = ctx.checker();
                let exported =
                    c.try_get_member_in_module_exports_and_properties(member, module_symbol);
                if let Some(exported) = exported
                    && (exported == symbol || {
                        let left = resolve_stability_alias(c, Some(exported));
                        let right = resolve_stability_alias(c, Some(symbol));
                        ets_typeparser::get_symbol_if_same_reference(c, left, right).is_some()
                    })
                {
                    result.allowed = true;
                    break;
                }
            }
        }
        result
    }
}

// go: internal/rules/stability_api_usage.go:288 stabilityApiModuleName
// This is a declaration path, not a reconstructed public import specifier.
pub(crate) fn stability_api_module_name(
    package_name: &str,
    directory: &str,
    file_name: &str,
) -> String {
    if directory.is_empty() {
        return String::new();
    }
    let normalized_directory = tspath::normalize_path(directory);
    let directory =
        format!("{}/", normalized_directory.strip_suffix('/').unwrap_or(&normalized_directory));
    let file_name = tspath::normalize_path(file_name);
    let Some(mut relative) = file_name.strip_prefix(&directory) else {
        return String::new();
    };
    for prefix in ["dist/dts/", "dist/esm/", "dist/cjs/", "src/", "dist/"] {
        if let Some(rest) = relative.strip_prefix(prefix) {
            relative = rest;
            break;
        }
    }
    for extension in
        [".d.ts", ".d.mts", ".d.cts", ".ts", ".mts", ".cts", ".tsx", ".js", ".mjs", ".cjs", ".jsx"]
    {
        if let Some(rest) = relative.strip_suffix(extension) {
            relative = rest;
            break;
        }
    }
    if tspath::get_base_file_name(relative) == "index" {
        let without_index = relative.strip_suffix("index").unwrap_or(relative);
        relative = without_index.strip_suffix('/').unwrap_or(without_index);
    }
    if relative.is_empty() {
        return package_name.to_string();
    }
    format!("{package_name}/{relative}")
}

// go: internal/rules/stability_api_usage.go:320 resolveStabilityAlias
// Synthetic default aliases have no alias declaration and cannot be resolved.
pub(crate) fn resolve_stability_alias(
    c: &mut Checker,
    mut symbol: Option<P<Symbol>>,
) -> Option<P<Symbol>> {
    let mut seen: FxHashSet<P<Symbol>> = FxHashSet::default();
    while let Some(s) = symbol
        && s.flags().intersects(SymbolFlags::Alias)
    {
        if seen.contains(&s) {
            return None;
        }
        seen.insert(s);
        if !s.declarations().iter().any(|&d| ast::is_alias_symbol_declaration(d)) {
            return None;
        }
        let next = ets_typeparser::api_stability_immediate_aliased_symbol(c, s);
        if next == Some(s) {
            break;
        }
        symbol = next;
    }
    symbol
}
