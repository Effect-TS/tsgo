// Ported from effect-tsgo's internal/typeparser/service_type_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

/// Rust-only: Go's `bundledeffect.EnsurePackageInstalled(version, "effect")`: the fixture profile mounts (its
/// packages are installed), see `mount_effect`.
fn ensure_effect_installed(version: EffectVersion) -> Result<(), String> {
    mount_effect(version, &mut TestFS::default())
}

// go test: internal/typeparser/service_type_test.go:11 TestServiceType_V4ServiceShape a7499ee1e0a3eb74
// Rust: the service's identifier and shape are non-nil types (`P<Type>`), so Go's nil checks of them always pass.
#[test]
fn test_service_type_v4_service_shape() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_effect_installed(EFFECT_V4) {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v4_internal(
            t,
            r#"
import { Context } from "effect"

export class UserRepo extends Context.Service<UserRepo, { readonly find: () => string }>()("UserRepo") {}
"#,
        );
        let mut tp = compiled.type_parser();

        let (_, class_type) = find_class_type_by_name(t, &mut tp, sf, "UserRepo");

        if tp.service_type(class_type).is_none() {
            t.fatal("expected v4 service type to parse");
        }
        if tp.context_tag(class_type).is_some() {
            t.fatal("expected v4 service not to parse as Context.Tag");
        }
    });
}

// go test: internal/typeparser/service_type_test.go:45 TestContextTag_V3LegacyShape 688c86e66f055665
// Rust: the tag's identifier and shape are non-nil types (`P<Type>`), so Go's nil checks of them always pass.
#[test]
fn test_context_tag_v3_legacy_shape() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_effect_installed(EFFECT_V3) {
            t.skip(sprintf!("Effect v3 not installed: %v", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import { Context } from "effect"

export class Config extends Context.Tag("Config")<Config, { readonly port: number }>() {}
"#,
        );
        let mut tp = compiled.type_parser();

        let (_, class_type) = find_class_type_by_name(t, &mut tp, sf, "Config");

        if tp.context_tag(class_type).is_none() {
            t.fatal("expected v3 Context.Tag type to parse");
        }
        if tp.service_type(class_type).is_some() {
            t.fatal("expected v3 Context.Tag not to parse as a v4 service type");
        }
    });
}

// go test: internal/typeparser/service_type_test.go:79 findClassTypeByName afdfa021aa182bc5
// Rust: Go builds a second parser over the checker (`NewTypeParser(c.Program(), c)`), which shares the checker's
// links; the caller's parser is used instead.
pub(crate) fn find_class_type_by_name(
    t: &T,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    class_name: &str,
) -> (P<Node>, P<Type>) {
    t.helper();

    fn walk(node: P<Node>, class_name: &str, found_name: &mut Option<P<Node>>) -> bool {
        if found_name.is_some() {
            return false;
        }
        if node.kind() == Kind::ClassDeclaration
            && let Some(name) = node.name()
            && name.text() == class_name
        {
            *found_name = Some(name);
            return false;
        }
        node.for_each_child(&mut |child| walk(child, class_name, found_name));
        false
    }
    let mut found_name = None;
    walk(sf.as_node(), class_name, &mut found_name);
    let Some(found_name) = found_name else {
        t.fatal(sprintf!("class %q not found", class_name));
    };

    let Some(class_sym) = tp.get_symbol_at_location(found_name) else {
        t.fatal(sprintf!("symbol for class %q not found", class_name));
    };
    let Some(class_type) = tp.checker.get_type_of_symbol_at_location(class_sym, Some(found_name))
    else {
        t.fatal(sprintf!("type for class %q not found", class_name));
    };
    (found_name, class_type)
}
