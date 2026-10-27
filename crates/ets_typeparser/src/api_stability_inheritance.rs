use ets_shim::ast::{Kind, Node, Symbol, SymbolFlags};
use ets_shim::core::P;
use rustc_hash::FxHashSet;

use crate::*;

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability_inheritance.go:14 apiStabilityAnalysis.optionalTaggedBase 0f8369605867c4be
    // optionalTaggedBase reads declaration metadata only. Every public member,
    // including inherited and merged members, must be an optional property or
    // method with its own recognized stability tag. Internal members are ignored.
    // Call, construct and index signatures cannot be optional. Unknown bases fail
    // closed; member types are
    // never evaluated merely to decide whether to exempt an inherited base.
    pub(crate) fn optional_tagged_base(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> bool {
        if self.optional_tagged_symbols.contains(&symbol) {
            return true;
        }
        let mut pending: Vec<Option<P<Symbol>>> = vec![symbol];
        let mut seen: FxHashSet<P<Symbol>> = FxHashSet::default();
        while let Some(current) = pending.pop() {
            let Some(current) = current else {
                return false;
            };
            if !current.flags().intersects(SymbolFlags::Interface | SymbolFlags::Class)
                || current.declarations().is_empty()
                || !self.consume_work()
            {
                return false;
            }
            if seen.contains(&current) {
                continue;
            }
            seen.insert(current);
            if current.flags().intersects(SymbolFlags::Class)
                && api_stability_symbol_declares_heritage(Some(current))
            {
                let declared =
                    get_resolved_declared_type_of_symbol_if_materialized(tp.checker, Some(current));
                if declared.is_none() {
                    return false;
                }
                if get_resolved_base_types_of_type_if_materialized(declared).is_none() {
                    return false;
                }
            }
            for &declaration in current.declarations() {
                let members: &'static [P<Node>] = match declaration.kind() {
                    Kind::InterfaceDeclaration => {
                        declaration.as_interface_declaration().members().nodes()
                    }
                    Kind::ClassDeclaration | Kind::ClassExpression => declaration
                        .class_like_data()
                        .map_or(&[], |class_like| class_like.members().nodes()),
                    _ => return false,
                };
                for &member in members {
                    if api_stability_symbol_has_only_internal_declarations(
                        get_symbol_of_declaration(tp.checker, member),
                    ) {
                        continue;
                    }
                    if !matches!(
                        member.kind(),
                        Kind::PropertySignature
                            | Kind::MethodSignature
                            | Kind::PropertyDeclaration
                            | Kind::MethodDeclaration
                    ) || !ets_shim::ast::has_question_token(member)
                        || stability_of_declaration_tag(Some(member)).is_empty()
                    {
                        return false;
                    }
                }
            }
            for &declaration in current.declarations() {
                if declaration.kind() != Kind::InterfaceDeclaration {
                    continue;
                }
                for &node in ets_shim::ast::get_extends_heritage_clause_elements(declaration) {
                    let name = match node.kind() {
                        Kind::TypeReference => node.as_type_reference_node().type_name(),
                        Kind::ExpressionWithTypeArguments => {
                            node.as_expression_with_type_arguments().expression()
                        }
                        _ => return false,
                    };
                    let mut base = self.symbol_at_type_name_node(tp, Some(name));
                    let mut aliases: FxHashSet<P<Symbol>> = FxHashSet::default();
                    while let Some(alias) = base
                        && alias.flags().intersects(SymbolFlags::Alias)
                        && !aliases.contains(&alias)
                    {
                        aliases.insert(alias);
                        base = api_stability_immediate_aliased_symbol(tp.checker, Some(alias));
                    }
                    pending.push(base);
                }
            }
            if current.flags().intersects(SymbolFlags::Class)
                && api_stability_symbol_declares_heritage(Some(current))
            {
                let declared =
                    get_resolved_declared_type_of_symbol_if_materialized(tp.checker, Some(current));
                let bases =
                    get_resolved_base_types_of_type_if_materialized(declared).unwrap_or(&[]);
                for &base in bases {
                    let Some(base_symbol) = base.symbol() else {
                        return false;
                    };
                    pending.push(Some(base_symbol));
                }
            }
        }
        self.optional_tagged_symbols.insert(symbol);
        true
    }

    // go: internal/typeparser/api_stability_inheritance.go:108 apiStabilityAnalysis.optionalTaggedInheritedMember f3f0ab77fdd5e4c0
    // optionalTaggedInheritedMember applies the base exception to properties the
    // compiler flattened into a derived member table, including superclass factory
    // intersection types. Own members are always inspected independently.
    pub(crate) fn optional_tagged_inherited_member(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: Option<P<Symbol>>,
        owner: Option<P<Symbol>>,
    ) -> bool {
        let declaring = self.member_declaring_symbol(tp, member);
        declaring.is_some()
            && owner.is_some()
            && declaring != owner
            && self.optional_tagged_base(tp, declaring)
    }
}
