use std::cell::RefCell;
use std::rc::Rc;

use ets_shim::ast::{self, Kind, Node, SourceFile};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::compiler::Program;
use ets_shim::core::context::Context;
use ets_shim::core::{P, TextPos};
use ets_shim::ls::{DocSym, DocSymData, LanguageService};
use ets_shim::lsproto;
use ets_shim::scanner;
use ets_typeparser::TypeParser;
use rustc_hash::FxHashSet;

// go: etslshooks/document_symbols.go:19 afterDocumentSymbols
//
// Rust: Go's `[]*lsproto.DocumentSymbol` is tsrs's shared symbol tree (`ls::DocSym`, tsrs patch 024), so the
// symbols added here can be merged and flattened by tsrs's code as Go's are.
pub(crate) fn after_document_symbols(
    ctx: &Context,
    sf: P<SourceFile>,
    symbols: Vec<DocSym>,
    program: &'static Program,
    lang_service: &LanguageService,
) -> Vec<DocSym> {
    if program.options().effect.get().is_none() {
        return symbols;
    }

    let mut c = program.get_type_checker_for_file(ctx, sf);
    let mut tp = ets_typeparser::new_type_parser(program, &mut c);

    let layer_children = collect_layer_document_symbols(&mut tp, sf, lang_service);
    let service_children = collect_service_document_symbols(&mut tp, sf, lang_service);
    let error_children = collect_error_document_symbols(&mut tp, sf, lang_service);
    let schema_children = collect_schema_document_symbols(&mut tp, sf, lang_service);
    if layer_children.is_empty()
        && service_children.is_empty()
        && error_children.is_empty()
        && schema_children.is_empty()
    {
        return symbols;
    }

    let mut effect_children: Vec<DocSym> = Vec::with_capacity(4);
    if !layer_children.is_empty() {
        let layers = new_synthetic_namespace_symbol("Layers");
        layers.borrow_mut().children = Some(layer_children);
        effect_children.push(layers);
    }
    if !service_children.is_empty() {
        let services = new_synthetic_namespace_symbol("Services");
        services.borrow_mut().children = Some(service_children);
        effect_children.push(services);
    }
    if !error_children.is_empty() {
        let errors = new_synthetic_namespace_symbol("Errors");
        errors.borrow_mut().children = Some(error_children);
        effect_children.push(errors);
    }
    if !schema_children.is_empty() {
        let schemas = new_synthetic_namespace_symbol("Schemas");
        schemas.borrow_mut().children = Some(schema_children);
        effect_children.push(schemas);
    }
    let effect = new_synthetic_namespace_symbol("Effect");
    effect.borrow_mut().children = Some(effect_children);

    let mut result = vec![effect];
    result.extend(symbols);
    result
}

/// Rust-only: what the recursive `walk` closures of Go's four `collect*DocumentSymbols` capture (each becomes a
/// nested `fn walk` taking it).
struct Walk<'s, 'l> {
    sf: P<SourceFile>,
    lang_service: &'l LanguageService,
    symbols: &'s mut Vec<DocSym>,
    seen: FxHashSet<P<Node>>,
}

// go: etslshooks/document_symbols.go:64 collectLayerDocumentSymbols
fn collect_layer_document_symbols(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    lang_service: &LanguageService,
) -> Vec<DocSym> {
    fn walk(tp: &mut TypeParser<'_>, w: &mut Walk<'_, '_>, current: P<Node>) -> bool {
        if is_effect_symbol_declaration(Some(current)) {
            if is_layer_declaration(tp, current) {
                let display_node = resolve_layer_display_node(current);
                if w.seen.insert(display_node) {
                    w.symbols.push(new_effect_document_symbol(
                        tp,
                        w.sf,
                        w.lang_service,
                        current,
                        display_node,
                        Some(layer_symbol_detail),
                    ));
                }
                return false;
            }
        }
        current.for_each_child(&mut |child| walk(tp, w, child));
        false
    }
    let mut symbols = Vec::new();
    let mut w = Walk { sf, lang_service, symbols: &mut symbols, seen: FxHashSet::default() };
    sf.as_node().for_each_child(&mut |child| walk(tp, &mut w, child));
    symbols
}

// go: etslshooks/document_symbols.go:89 collectServiceDocumentSymbols
fn collect_service_document_symbols(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    lang_service: &LanguageService,
) -> Vec<DocSym> {
    fn walk(tp: &mut TypeParser<'_>, w: &mut Walk<'_, '_>, current: P<Node>) -> bool {
        if is_effect_symbol_declaration(Some(current)) {
            if is_service_declaration(tp, Some(current)) {
                let display_node = resolve_service_display_node(current);
                if w.seen.insert(display_node) {
                    w.symbols.push(new_effect_document_symbol(
                        tp,
                        w.sf,
                        w.lang_service,
                        current,
                        display_node,
                        None,
                    ));
                }
                return false;
            }
            let t = tp.get_type_at_location(current);
            if tp.is_layer_type(t) {
                return false;
            }
        }
        current.for_each_child(&mut |child| walk(tp, w, child));
        false
    }
    let mut symbols = Vec::new();
    let mut w = Walk { sf, lang_service, symbols: &mut symbols, seen: FxHashSet::default() };
    sf.as_node().for_each_child(&mut |child| walk(tp, &mut w, child));
    symbols
}

// go: etslshooks/document_symbols.go:118 collectErrorDocumentSymbols
fn collect_error_document_symbols(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    lang_service: &LanguageService,
) -> Vec<DocSym> {
    fn walk(tp: &mut TypeParser<'_>, w: &mut Walk<'_, '_>, current: P<Node>) -> bool {
        if is_effect_symbol_declaration(Some(current)) {
            if is_error_declaration(tp, Some(current)) {
                let display_node = resolve_error_display_node(current);
                if w.seen.insert(display_node) {
                    w.symbols.push(new_effect_document_symbol(
                        tp,
                        w.sf,
                        w.lang_service,
                        current,
                        display_node,
                        None,
                    ));
                }
                return false;
            }
        }
        current.for_each_child(&mut |child| walk(tp, w, child));
        false
    }
    let mut symbols = Vec::new();
    let mut w = Walk { sf, lang_service, symbols: &mut symbols, seen: FxHashSet::default() };
    sf.as_node().for_each_child(&mut |child| walk(tp, &mut w, child));
    symbols
}

// go: etslshooks/document_symbols.go:143 collectSchemaDocumentSymbols
fn collect_schema_document_symbols(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    lang_service: &LanguageService,
) -> Vec<DocSym> {
    fn walk(tp: &mut TypeParser<'_>, w: &mut Walk<'_, '_>, current: P<Node>) -> bool {
        if is_effect_symbol_declaration(Some(current)) {
            if is_schema_declaration(tp, Some(current)) {
                let display_node = resolve_schema_display_node(current);
                if w.seen.insert(display_node) {
                    w.symbols.push(new_effect_document_symbol(
                        tp,
                        w.sf,
                        w.lang_service,
                        current,
                        display_node,
                        None,
                    ));
                }
                return false;
            }
        }
        current.for_each_child(&mut |child| walk(tp, w, child));
        false
    }
    let mut symbols = Vec::new();
    let mut w = Walk { sf, lang_service, symbols: &mut symbols, seen: FxHashSet::default() };
    sf.as_node().for_each_child(&mut |child| walk(tp, &mut w, child));
    symbols
}

// go: etslshooks/document_symbols.go:168 newSyntheticNamespaceSymbol
fn new_synthetic_namespace_symbol(name: &str) -> DocSym {
    let children: Vec<DocSym> = Vec::new();
    let zero = lsproto::Position::default();
    Rc::new(RefCell::new(DocSymData {
        name: name.to_string(),
        detail: None,
        kind: lsproto::SymbolKind::PACKAGE,
        range: lsproto::Range { start: zero, end: zero },
        selection_range: lsproto::Range { start: zero, end: zero },
        children: Some(children),
    }))
}

// go: etslshooks/document_symbols.go:186 newNamedDocumentSymbol
fn new_named_document_symbol(
    sf: P<SourceFile>,
    lang_service: &LanguageService,
    node: Option<P<Node>>,
    name: String,
    detail: Option<String>,
    kind: lsproto::SymbolKind,
) -> DocSym {
    let children: Vec<DocSym> = Vec::new();
    let Some(node) = node else {
        let zero = lsproto::Position::default();
        return Rc::new(RefCell::new(DocSymData {
            name,
            detail,
            kind,
            range: lsproto::Range { start: zero, end: zero },
            selection_range: lsproto::Range { start: zero, end: zero },
            children: Some(children),
        }));
    };

    // Go's shim accessor `ls.LanguageService_converters` (tsrs patch 100).
    let converters = &lang_service.converters;
    let start_pos =
        scanner::skip_trivia(sf.text(), node.pos().expect_source("a declaration of the file"));
    let end_pos = start_pos.max(node.end().expect_source("a declaration of the file"));
    let (start, _) = converters.to_lsp_position(&*sf, TextPos::from(start_pos));
    let (end, _) = converters.to_lsp_position(&*sf, TextPos::from(end_pos));

    Rc::new(RefCell::new(DocSymData {
        name,
        detail,
        kind,
        range: lsproto::Range { start, end },
        selection_range: lsproto::Range { start, end },
        children: Some(children),
    }))
}

/// Rust-only: Go's `detail func(*typeparser.TypeParser, *checker.Checker, *ast.Node) *string` parameter (the
/// checker is the parser's).
type DetailFn = fn(&mut TypeParser<'_>, P<Node>) -> Option<String>;

// go: etslshooks/document_symbols.go:229 newEffectDocumentSymbol
fn new_effect_document_symbol(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    lang_service: &LanguageService,
    node: P<Node>,
    display_node: P<Node>,
    detail: Option<DetailFn>,
) -> DocSym {
    let children: Vec<DocSym> = Vec::new();
    let mut symbol_detail: Option<String> = None;
    if let Some(detail) = detail {
        symbol_detail = detail(tp, node);
    }

    let symbol = new_named_document_symbol(
        sf,
        lang_service,
        Some(node),
        layer_symbol_name(sf, display_node),
        symbol_detail,
        layer_symbol_kind(display_node),
    );
    symbol.borrow_mut().children = Some(children);
    symbol
}

// go: etslshooks/document_symbols.go:249 layerSymbolDetail
fn layer_symbol_detail(tp: &mut TypeParser<'_>, node: P<Node>) -> Option<String> {
    let (type_check_node, types) = classification_types(tp, Some(node));
    for t in types {
        let Some(layer) = tp.layer_type(t) else {
            continue;
        };
        let c = tp.checker();
        let r_out = c.type_to_string_ex(
            layer.r_out(),
            type_check_node,
            ets_shim::checker::types::TypeFormatFlags::NoTruncation,
            None,
        );
        let e = c.type_to_string_ex(
            layer.e(),
            type_check_node,
            ets_shim::checker::types::TypeFormatFlags::NoTruncation,
            None,
        );
        let r_in = c.type_to_string_ex(
            layer.r_in(),
            type_check_node,
            ets_shim::checker::types::TypeFormatFlags::NoTruncation,
            None,
        );
        let detail = "<".to_string() + &r_out + ", " + &e + ", " + &r_in + ">";
        return Some(detail);
    }
    None
}

// go: etslshooks/document_symbols.go:265 resolveLayerDisplayNode
fn resolve_layer_display_node(node: P<Node>) -> P<Node> {
    let Some(parent) = node.parent() else {
        return node;
    };
    match parent.kind() {
        Kind::VariableDeclaration
        | Kind::PropertyDeclaration
        | Kind::PropertyAssignment
        | Kind::ShorthandPropertyAssignment
        | Kind::PropertySignature
        | Kind::BindingElement => parent,
        _ => node,
    }
}

// go: etslshooks/document_symbols.go:282 resolveServiceDisplayNode
fn resolve_service_display_node(node: P<Node>) -> P<Node> {
    let mut current = Some(node);
    while let Some(c) = current {
        match c.kind() {
            Kind::ClassDeclaration | Kind::VariableDeclaration | Kind::PropertyDeclaration => {
                return c;
            }
            _ => {}
        }
        current = c.parent();
    }
    node
}

// go: etslshooks/document_symbols.go:294 classificationTypes
//
// Rust: Go's checker parameter is the parser's (`tp.checker()`).
fn classification_types(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> (Option<P<Node>>, Vec<P<Type>>) {
    let Some(node) = node else {
        return (None, Vec::new());
    };
    let Some(t) = tp.get_type_at_location(node) else {
        return (Some(node), Vec::new());
    };
    let mut types = vec![t];
    if node.kind() == Kind::ClassDeclaration {
        if let Some(class_name) = node.name() {
            if let Some(class_symbol) = tp.get_symbol_at_location(class_name) {
                if let Some(class_type) =
                    tp.checker().get_type_of_symbol_at_location(class_symbol, Some(node))
                {
                    if class_type != t {
                        types.push(class_type);
                    }
                }
            }
        }
    }
    let c = tp.checker();
    let construct_signatures = c.get_construct_signatures(t);
    if let Some(&first) = construct_signatures.first() {
        // Go: `if returnType := ...; returnType != nil`; tsrs's return type is never nil.
        let return_type = c.get_return_type_of_signature(first);
        types.push(return_type);
    }
    let call_signatures = c.get_signatures_of_type(t, SignatureKind::Call);
    if let Some(&first) = call_signatures.first() {
        let return_type = c.get_return_type_of_signature(first);
        types.push(return_type);
    }
    (Some(node), types)
}

// go: etslshooks/document_symbols.go:325 isLayerDeclaration
fn is_layer_declaration(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let (_, types) = classification_types(tp, Some(node));
    types.into_iter().any(|t| tp.is_layer_type(t))
}

// go: etslshooks/document_symbols.go:330 isServiceDeclaration
fn is_service_declaration(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    if node.is_none() {
        return false;
    }
    let (_, types) = classification_types(tp, node);
    for t in types {
        if tp.is_service_type(t) || tp.is_context_tag(t) {
            return true;
        }
    }
    false
}

// go: etslshooks/document_symbols.go:343 isErrorDeclaration
fn is_error_declaration(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(n) = node else {
        return false;
    };
    match n.kind() {
        Kind::ClassDeclaration | Kind::VariableDeclaration => {}
        _ => return false,
    }
    let (_, types) = classification_types(tp, node);
    types.into_iter().any(|t| tp.is_yieldable_error_type(t))
}

// go: etslshooks/document_symbols.go:356 isSchemaDeclaration
fn is_schema_declaration(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(n) = node else {
        return false;
    };
    match n.kind() {
        Kind::ClassDeclaration | Kind::VariableDeclaration | Kind::PropertyDeclaration => {}
        _ => return false,
    }
    let (_, types) = classification_types(tp, node);
    types.into_iter().any(|t| tp.is_schema_type(t))
}

// go: etslshooks/document_symbols.go:369 resolveErrorDisplayNode
fn resolve_error_display_node(node: P<Node>) -> P<Node> {
    let mut current = Some(node);
    while let Some(c) = current {
        match c.kind() {
            Kind::ClassDeclaration | Kind::VariableDeclaration | Kind::PropertyDeclaration => {
                return c;
            }
            _ => {}
        }
        current = c.parent();
    }
    node
}

// go: etslshooks/document_symbols.go:381 resolveSchemaDisplayNode
fn resolve_schema_display_node(node: P<Node>) -> P<Node> {
    let mut current = Some(node);
    while let Some(c) = current {
        match c.kind() {
            Kind::ClassDeclaration | Kind::VariableDeclaration | Kind::PropertyDeclaration => {
                return c;
            }
            _ => {}
        }
        current = c.parent();
    }
    node
}

// go: etslshooks/document_symbols.go:393 isEffectSymbolDeclaration
fn is_effect_symbol_declaration(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    match node.kind() {
        Kind::ClassDeclaration | Kind::VariableDeclaration | Kind::PropertyDeclaration => {}
        _ => return false,
    }
    let mut current = node.parent();
    while let Some(c) = current {
        if c.kind() == Kind::ObjectLiteralExpression
            || c.kind() == Kind::ForOfStatement
            || c.kind() == Kind::ForInStatement
        {
            return false;
        }
        current = c.parent();
    }
    true
}

// go: etslshooks/document_symbols.go:412 layerSymbolName
//
// Rust: Go's `scanner.GetTextOfNode(nil)` (a class expression without a name) panics; here it is `""`. Go's
// `text[:77]` cuts bytes; a cut inside a multi-byte character keeps the character's leading bytes in Go and writes
// U+FFFD here (`docs/DIVERGENCES.md`, the layergraph truncation row).
fn layer_symbol_name(sf: P<SourceFile>, node: P<Node>) -> String {
    let text_of = |n: Option<P<Node>>| n.map(scanner::get_text_of_node).unwrap_or_default();
    if node.kind() == Kind::PropertyDeclaration {
        if let Some(class_like) = node.parent().filter(|p| ast::is_class_like(*p)) {
            let class_name = ets_go::strings::trim_space(&text_of(class_like.name())).to_string();
            let property_name = ets_go::strings::trim_space(&text_of(node.name())).to_string();
            if !class_name.is_empty() && !property_name.is_empty() {
                return class_name + "." + &property_name;
            }
        }
    }
    if ast::is_declaration(node) {
        if let Some(name) = ast::get_name_of_declaration(node) {
            let text = scanner::get_text_of_node(name);
            let text = ets_go::strings::trim_space(&text);
            if !text.is_empty() {
                return text.to_string();
            }
        }
    }
    let text = scanner::get_source_text_of_node_from_source_file(sf, node, false);
    let text = ets_go::strings::trim_space(&text);
    if text.is_empty() {
        return "<layer>".to_string();
    }
    if text.len() > 80 {
        return String::from_utf8_lossy(&text.as_bytes()[..77]).into_owned() + "...";
    }
    text.to_string()
}

// go: etslshooks/document_symbols.go:440 layerSymbolKind
fn layer_symbol_kind(node: P<Node>) -> lsproto::SymbolKind {
    match node.kind() {
        Kind::VariableDeclaration | Kind::BindingElement => lsproto::SymbolKind::VARIABLE,
        Kind::PropertyDeclaration | Kind::PropertyAssignment | Kind::PropertySignature => {
            lsproto::SymbolKind::PROPERTY
        }
        Kind::FunctionDeclaration
        | Kind::FunctionExpression
        | Kind::ArrowFunction
        | Kind::MethodDeclaration => lsproto::SymbolKind::FUNCTION,
        Kind::ClassDeclaration | Kind::ClassExpression => lsproto::SymbolKind::CLASS,
        _ => lsproto::SymbolKind::VARIABLE,
    }
}
