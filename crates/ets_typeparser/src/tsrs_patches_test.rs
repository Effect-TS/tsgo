// Rust-only: regression tests for the tsrs patches in _patches/tsrs (no Go counterpart): each fails without the
// patch it names.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ets_shim::checker::{
    RelationError, new_diagnostic_for_node, register_after_check_source_file_callback,
};
use ets_shim::core::context::Context;

use crate::helpers_test::{compile_and_get_checker_and_source_file_internal, run};

const MARKER: &str = "tsrsPatchesHookMarker";

// The callback is process-wide: it acts only on this test's file (the marker), so the other tests that check
// files in this binary see no difference.
static CALLS: AtomicUsize = AtomicUsize::new(0);
static RELATION_ERRORS_IN_CALLBACK: AtomicUsize = AtomicUsize::new(0);

// _patches/tsrs/002 (hook, AddDiagnostic, Program, GetRelationErrors, IsSourceFileFromExternalLibrary),
// 004 (relation errors are collected) and 005 (RelationError, SourceFileLinks.relation_errors).
#[test]
fn test_after_check_source_file_callback() {
    run(|t| {
        let _guard = ets_go::testing::lock_after_check_callback();
        register_after_check_source_file_callback(Arc::new(|_ctx, program, checker, sf| {
            if !sf.text().contains(MARKER) {
                return;
            }
            CALLS.fetch_add(1, Ordering::SeqCst);
            assert!(!program.is_source_file_from_external_library(sf));
            assert!(std::ptr::addr_eq(program, checker.program()));
            let relation_errors: Vec<RelationError> = checker.get_relation_errors(sf);
            RELATION_ERRORS_IN_CALLBACK.store(relation_errors.len(), Ordering::SeqCst);
            checker.add_diagnostic_exported(new_diagnostic_for_node(
                Some(sf.as_node()),
                Some(&ets_shim::diagnostics::Cannot_find_name_0),
                &[&MARKER],
            ));
        }));

        let source = format!("const {MARKER} = 1;\nconst x: string = 1;\n");
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, &source);
        let ctx = Context::background();
        let diagnostics = compiled.checker.get_diagnostics_exported(&ctx, sf);
        // Checking again runs no second callback: the file is already checked.
        compiled.checker.get_diagnostics_exported(&ctx, sf);

        if CALLS.load(Ordering::SeqCst) != 1 {
            t.fatal(ets_go::sprintf!(
                "expected 1 callback call, got %d",
                CALLS.load(Ordering::SeqCst) as i64
            ));
        }
        let codes: Vec<i32> = diagnostics.iter().map(|d| d.code()).collect();
        if !codes.contains(&2322) || !codes.contains(&2304) {
            t.error(ets_go::sprintf!("expected diagnostics 2322 and 2304, got %v", codes));
        }
        if RELATION_ERRORS_IN_CALLBACK.load(Ordering::SeqCst) != 1 {
            t.error(ets_go::sprintf!(
                "expected 1 relation error in the callback, got %d",
                RELATION_ERRORS_IN_CALLBACK.load(Ordering::SeqCst) as i64
            ));
        }
        let relation_errors = compiled.checker.get_relation_errors(sf);
        let [relation_error] = relation_errors.as_slice() else {
            t.fatal(ets_go::sprintf!(
                "expected 1 relation error, got %d",
                relation_errors.len() as i64
            ));
        };
        let mut tp = compiled.type_parser();
        let source_type = tp.checker().type_to_string_exported(relation_error.source);
        let target_type = tp.checker().type_to_string_exported(relation_error.target);
        // The relation is checked on the literal type (the error message generalizes it to `number`).
        if source_type != "1" || target_type != "string" {
            t.error(ets_go::sprintf!(
                "expected 1 -> string, got %s -> %s",
                source_type,
                target_type
            ));
        }
    });
}
