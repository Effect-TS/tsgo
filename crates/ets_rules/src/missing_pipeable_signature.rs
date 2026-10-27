use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, Symbol, SymbolFlags};
use ets_shim::checker::{Signature, SignatureKind, TypeFormatFlags};
use ets_shim::diagnostics as tsdiag;
use rustc_hash::FxHashSet;

// go: internal/rules/missing_pipeable_signature.go:12 MissingPipeableSignature 3a4da10e3aca0b7f
pub static MISSING_PIPEABLE_SIGNATURE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "missingPipeableSignature",
        group: "style",
        description: "Reports exported fixed-arity functions whose call signatures have no corresponding pipeable overload",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![
            tsdiag::EXPORTED_FUNCTION_0_HAS_NO_PIPEABLE_OVERLOAD_CORRESPONDING_TO_ITS_SIGNATURE_1_EFFECT_MISSING_PIPEABLE_SIGNATURE.code(),
        ],
        run: check_missing_pipeable_signatures,
    }
});

// go: internal/rules/missing_pipeable_signature.go:24 checkMissingPipeableSignatures 3669592fc991488f
//
// Rust: Go's `map[*checker.Signature]bool` sets are `FxHashSet`s of the signature handles (pointer identity).
pub(crate) fn check_missing_pipeable_signatures(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    let source_file = ctx.source_file;
    let Some(module_symbol) =
        ets_typeparser::get_symbol_of_declaration(ctx.checker(), source_file.as_node())
    else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for export_symbol in ctx.checker().get_exports_of_module_exported(module_symbol) {
        let (Some(target), Some(location)) = local_export_target(ctx, Some(export_symbol)) else {
            continue;
        };

        let Some(export_type) =
            ctx.checker().get_type_of_symbol_at_location(target, Some(location))
        else {
            continue;
        };
        let signatures = ctx.checker().get_signatures_of_type(export_type, SignatureKind::Call);
        if signatures.is_empty() {
            continue;
        }

        let mut pipeable_targets: FxHashSet<P<Signature>> = FxHashSet::default();
        let mut has_pipeable_signature: FxHashSet<P<Signature>> = FxHashSet::default();
        for &data_first in signatures {
            if !is_eligible_data_first_signature(Some(data_first)) {
                continue;
            }
            let params = data_first.parameters();
            for subject_index in [0, params.len() as i64 - 1] {
                for &candidate in signatures {
                    if candidate == data_first {
                        continue;
                    }
                    if ets_typeparser::matches_pipeable_signature(
                        ctx.type_parser,
                        Some(data_first),
                        Some(candidate),
                        subject_index,
                        None,
                    ) {
                        has_pipeable_signature.insert(data_first);
                        pipeable_targets.insert(candidate);
                    }
                }
            }
        }

        for &signature in signatures {
            if !is_eligible_data_first_signature(Some(signature))
                || pipeable_targets.contains(&signature)
                || has_pipeable_signature.contains(&signature)
            {
                continue;
            }
            let signature_text = ctx.checker().signature_to_string_ex(
                signature,
                Some(location),
                TypeFormatFlags::WriteArrowStyleSignature,
                None,
            );
            diagnostics.push(ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(location),
                &tsdiag::EXPORTED_FUNCTION_0_HAS_NO_PIPEABLE_OVERLOAD_CORRESPONDING_TO_ITS_SIGNATURE_1_EFFECT_MISSING_PIPEABLE_SIGNATURE,
                &[],
                &[export_symbol.name(), &signature_text],
            ));
        }
    }

    diagnostics
}

// go: internal/rules/missing_pipeable_signature.go:82 localExportTarget 1c47a156a671c482
pub(crate) fn local_export_target(
    ctx: &mut Context<'_, '_>,
    export_symbol: Option<P<Symbol>>,
) -> (Option<P<Symbol>>, Option<P<Node>>) {
    let Some(export_symbol) = export_symbol else {
        return (None, None);
    };

    let mut target = export_symbol;
    if target.flags().intersects(SymbolFlags::Alias) {
        target = ctx.checker().get_aliased_symbol(target);
    }

    let mut target_declaration = None;
    for &declaration in target.declarations() {
        if ast::get_source_file_of_node(declaration) == Some(ctx.source_file) {
            target_declaration = Some(declaration);
            break;
        }
    }
    let Some(target_declaration) = target_declaration else {
        return (None, None);
    };

    let mut location = ast::get_name_of_declaration(target.value_declaration());
    if location.is_none() {
        location = ast::get_name_of_declaration(target_declaration);
    }
    if location.is_none() {
        location = Some(target_declaration);
    }
    (Some(target), location)
}

// go: internal/rules/missing_pipeable_signature.go:116 isEligibleDataFirstSignature 79131cc4f6ad805c
pub(crate) fn is_eligible_data_first_signature(signature: Option<P<Signature>>) -> bool {
    signature.is_some_and(|signature| {
        !signature.has_rest_parameter() && signature.parameters().len() >= 2
    })
}
