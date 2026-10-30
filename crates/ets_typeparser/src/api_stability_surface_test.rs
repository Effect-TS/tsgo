// Ported from effect-tsgo's internal/typeparser/api_stability_surface_test.go (see helpers_test.rs).

use ets_go::testing::T;
use ets_shim::ast::{SourceFile, Symbol};
use ets_shim::checker::{Signature, Type};
use ets_shim::core::P;

use crate::api_stability_test::*;
use crate::helpers_test::*;
use crate::*;

/// Rust-only: Go's `apiStabilityTestExport(t, c, file, name)` through the parser's checker.
fn export_of(t: &T, tp: &mut TypeParser<'_>, sf: P<SourceFile>, name: &str) -> P<Symbol> {
    api_stability_test_export(t, tp.checker, sf, name)
}

/// Rust-only: Go's `session.UsedBySymbol(apiStabilityTestExport(t, c, file, name))`, the tests' `surface`
/// closures.
fn used_by_name(
    t: &T,
    session: &mut ApiStabilitySession,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    name: &str,
) -> ApiStabilityUsed {
    let symbol = export_of(t, tp, sf, name);
    session.used_by_symbol(tp, symbol)
}

/// Rust-only: Go's `%v` of an `ApiStabilityLevel` (its `int`).
fn lv(level: ApiStabilityLevel) -> i64 {
    level as i64
}

/// Rust-only: Go's `%v` of the dependency names.
fn names(used: &ApiStabilityUsed) -> String {
    format!("{:?}", api_stability_dependency_names(used))
}

// go test: internal/typeparser/api_stability_surface_test.go:10 assertApiStabilitySurfaceExcludesOwnTag 6902dd7861d46d45
pub(crate) fn assert_api_stability_surface_excludes_own_tag(
    t: &T,
    used: &ApiStabilityUsed,
    name: &str,
) {
    t.helper();
    for dependency in &used.dependencies {
        if api_stability_dependency_name(dependency) == name {
            t.fatal(format!(
                "root tag {name:?} leaked into its own surface result: {}",
                names(used)
            ));
        }
    }
}

// go test: internal/typeparser/api_stability_surface_test.go:19 TestApiStabilityRootTagIsExcludedFromResult 69ac81f3b782cc51
#[test]
fn test_api_stability_root_tag_is_excluded_from_result() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }

/** @stability unstable */
export interface TaggedRoot { value: string }

/** @stability experimental */
export interface TaggedExperimentalRoot { value: string }

/** @stability stable */
export interface ExplicitStableRoot { value: string }

/** @stability unstable */
export interface TaggedRootWithChild { member: E }

/** @stability unstable */
export function taggedFn(x: string): string

/** @stability unstable */
export function taggedLeaky(): E

/** @stability unstable */
export type TaggedAlias = { value: string }

/** @stability unstable */
export type TaggedAliasLeaky = { value: E }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let mut session = tp.new_api_stability_session();

        let tagged_root = export_of(t, &mut tp, test_file, "TaggedRoot");
        let tagged_root_declared = tp.declared_api_stability_of_symbol(tagged_root);
        if tagged_root_declared.level != ApiStabilityLevel::Unstable
            || tagged_root_declared.declaration.is_none()
        {
            t.fatal(format!(
                "TaggedRoot declared = {tagged_root_declared:?}, want unstable with provenance"
            ));
        }
        let symbol = export_of(t, &mut tp, test_file, "TaggedExperimentalRoot");
        let experimental_declared = tp.declared_api_stability_of_symbol(symbol);
        if experimental_declared.level != ApiStabilityLevel::Experimental
            || experimental_declared.declaration.is_none()
        {
            t.fatal(format!(
                "TaggedExperimentalRoot declared = {experimental_declared:?}, want experimental with provenance"
            ));
        }
        let symbol = export_of(t, &mut tp, test_file, "ExplicitStableRoot");
        let explicit_stable_declared = tp.declared_api_stability_of_symbol(symbol);
        if explicit_stable_declared.level != ApiStabilityLevel::Stable
            || explicit_stable_declared.declaration.is_none()
        {
            t.fatal(format!(
                "an explicit stable root must stay distinct from an absent tag: {explicit_stable_declared:?}"
            ));
        }

        let tagged_root_type = tp.checker.get_declared_type_of_symbol_exported(tagged_root);
        let tag_only: Vec<(&str, ApiStabilityUsed)> = vec![
            ("TaggedRoot", session.used_by_symbol(&mut tp, tagged_root)),
            ("TaggedRoot type", session.used_by_type(&mut tp, tagged_root_type)),
            ("taggedFn", used_by_name(t, &mut session, &mut tp, test_file, "taggedFn")),
            ("TaggedAlias", used_by_name(t, &mut session, &mut tp, test_file, "TaggedAlias")),
            (
                "TaggedExperimentalRoot",
                used_by_name(t, &mut session, &mut tp, test_file, "TaggedExperimentalRoot"),
            ),
            (
                "ExplicitStableRoot",
                used_by_name(t, &mut session, &mut tp, test_file, "ExplicitStableRoot"),
            ),
        ];
        for (name, used) in &tag_only {
            if used.incomplete {
                t.fatal(format!(
                    "{name}: a leaf surface with only an own tag must settle complete"
                ));
            }
            if used.minimum != ApiStabilityLevel::Stable || !used.dependencies.is_empty() {
                t.fatal(format!(
                    "{name}: own tag must be excluded, got min={} deps={}",
                    lv(used.minimum),
                    names(used)
                ));
            }
        }

        let tagged_fn = export_of(t, &mut tp, test_file, "taggedFn");
        let signature = api_stability_test_signatures(t, tp.checker, Some(tagged_fn))[0];
        let fn_declared = tp.declared_api_stability_of_signature(signature);
        if fn_declared.level != ApiStabilityLevel::Unstable || fn_declared.declaration.is_none() {
            t.fatal(format!(
                "taggedFn signature declared = {fn_declared:?}, want unstable with provenance"
            ));
        }
        let signature = api_stability_test_signatures(t, tp.checker, Some(tagged_fn))[0];
        let fn_used = session.used_by_signature(&mut tp, signature);
        if fn_used.incomplete
            || fn_used.minimum != ApiStabilityLevel::Stable
            || !fn_used.dependencies.is_empty()
        {
            t.fatal(format!(
                "taggedFn signature own tag must be excluded, got min={} deps={}",
                lv(fn_used.minimum),
                names(&fn_used)
            ));
        }

        let with_child = used_by_name(t, &mut session, &mut tp, test_file, "TaggedRootWithChild");
        if with_child.incomplete
            || with_child.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&with_child, "E")
        {
            t.fatal(format!(
                "TaggedRootWithChild should expose E, got min={} deps={}",
                lv(with_child.minimum),
                names(&with_child)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &with_child, "TaggedRootWithChild");

        let leaky = used_by_name(t, &mut session, &mut tp, test_file, "taggedLeaky");
        if leaky.incomplete
            || leaky.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&leaky, "E")
        {
            t.fatal(format!(
                "taggedLeaky should expose E, got min={} deps={}",
                lv(leaky.minimum),
                names(&leaky)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &leaky, "taggedLeaky");

        let alias_leaky = used_by_name(t, &mut session, &mut tp, test_file, "TaggedAliasLeaky");
        if alias_leaky.incomplete
            || alias_leaky.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&alias_leaky, "E")
        {
            t.fatal(format!(
                "TaggedAliasLeaky should expose E, got min={} deps={}",
                lv(alias_leaky.minimum),
                names(&alias_leaky)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &alias_leaky, "TaggedAliasLeaky");

        let alias_symbol = export_of(t, &mut tp, test_file, "TaggedAliasLeaky");
        let alias_declared = tp.checker.get_declared_type_of_symbol_exported(alias_symbol);
        let alias_type = session.used_by_type(&mut tp, alias_declared);
        if alias_type.incomplete
            || alias_type.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&alias_type, "E")
        {
            t.fatal(format!(
                "TaggedAliasLeaky type should expose E, got min={} deps={}",
                lv(alias_type.minimum),
                names(&alias_type)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &alias_type, "TaggedAliasLeaky");
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:137 TestApiStabilityRootExclusionIsAResultConversion 5bde309bfc5640e8
#[test]
fn test_api_stability_root_exclusion_is_a_result_conversion() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface Shared { value: string }
export interface Holder { shared: Shared }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let shared = export_of(t, &mut tp, test_file, "Shared");
        let holder = export_of(t, &mut tp, test_file, "Holder");

        let check_shared =
            |t: &T, session: &mut ApiStabilitySession, tp: &mut TypeParser<'_>, label: &str| {
                t.helper();
                let used = session.used_by_symbol(tp, shared);
                if used.incomplete
                    || used.minimum != ApiStabilityLevel::Stable
                    || !used.dependencies.is_empty()
                {
                    t.fatal(format!(
                        "{label}: Shared as a root must exclude its own tag, got min={} deps={}",
                        lv(used.minimum),
                        names(&used)
                    ));
                }
                let stored = session.analysis.session_symbols.get(&shared);
                if stored.is_none_or(|surface| surface.findings.is_empty()) {
                    t.fatal(format!(
                        "{label}: the stored surface must keep the own tag for child reuse"
                    ));
                }
            };

        let check_holder =
            |t: &T, session: &mut ApiStabilitySession, tp: &mut TypeParser<'_>, label: &str| {
                t.helper();
                let used = session.used_by_symbol(tp, holder);
                if used.incomplete
                    || used.minimum != ApiStabilityLevel::Experimental
                    || !contains_dependency(&used, "Shared")
                {
                    t.fatal(format!(
                        "{label}: Holder should expose Shared as a child, got min={} deps={}",
                        lv(used.minimum),
                        names(&used)
                    ));
                }
                let stored = session.analysis.session_symbols.get(&holder);
                if stored.is_none_or(|surface| surface.findings.is_empty()) {
                    t.fatal(format!("{label}: Holder's stored surface must contain the child tag"));
                }
            };

        let mut child_last = tp.new_api_stability_session();
        check_shared(t, &mut child_last, &mut tp, "shared-first");
        check_holder(t, &mut child_last, &mut tp, "shared-first");
        check_shared(t, &mut child_last, &mut tp, "shared-first-repeat");

        let mut root_last = tp.new_api_stability_session();
        check_holder(t, &mut root_last, &mut tp, "child-first");
        check_shared(t, &mut root_last, &mut tp, "child-first");
        check_holder(t, &mut root_last, &mut tp, "child-first-repeat");
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:188 TestApiStabilityAliasRootVersusConcreteIdentity a21ce11000e53434
#[test]
fn test_api_stability_alias_root_versus_concrete_identity() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability unstable */
export interface Concrete { value: string }
export type AliasToConcrete = Concrete
/** @stability unstable */
export type TaggedAliasToConcrete = Concrete
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let mut session = tp.new_api_stability_session();

        let concrete = export_of(t, &mut tp, test_file, "Concrete");
        let concrete_used = session.used_by_symbol(&mut tp, concrete);
        if concrete_used.incomplete
            || concrete_used.minimum != ApiStabilityLevel::Stable
            || !concrete_used.dependencies.is_empty()
        {
            t.fatal(format!(
                "Concrete as a root must exclude its own tag, got min={} deps={}",
                lv(concrete_used.minimum),
                names(&concrete_used)
            ));
        }
        let concrete_declared = tp.checker.get_declared_type_of_symbol_exported(concrete);
        let concrete_type = session.used_by_type(&mut tp, concrete_declared);
        if concrete_type.incomplete
            || concrete_type.minimum != ApiStabilityLevel::Stable
            || !concrete_type.dependencies.is_empty()
        {
            t.fatal(format!(
                "Concrete's type must exclude its own tag, got min={} deps={}",
                lv(concrete_type.minimum),
                names(&concrete_type)
            ));
        }

        let plain = export_of(t, &mut tp, test_file, "AliasToConcrete");
        let plain_used = session.used_by_symbol(&mut tp, plain);
        if plain_used.incomplete
            || plain_used.minimum != ApiStabilityLevel::Unstable
            || !contains_dependency(&plain_used, "Concrete")
        {
            t.fatal(format!(
                "AliasToConcrete should expose Concrete as a child, got min={} deps={}",
                lv(plain_used.minimum),
                names(&plain_used)
            ));
        }

        let tagged_alias = export_of(t, &mut tp, test_file, "TaggedAliasToConcrete");
        let tagged_alias_used = session.used_by_symbol(&mut tp, tagged_alias);
        if tagged_alias_used.incomplete
            || tagged_alias_used.minimum != ApiStabilityLevel::Unstable
            || !contains_dependency(&tagged_alias_used, "Concrete")
        {
            t.fatal(format!(
                "TaggedAliasToConcrete should exclude only its own tag and expose Concrete, got min={} deps={}",
                lv(tagged_alias_used.minimum),
                names(&tagged_alias_used)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(
            t,
            &tagged_alias_used,
            "TaggedAliasToConcrete",
        );

        // The compiler represents `TaggedAliasToConcrete` as `Concrete`'s own type,
        // so the type-level query sees `Concrete` as the root identity and excludes
        // it. The alias symbol is not part of the type object.
        let tagged_alias_declared = tp.checker.get_declared_type_of_symbol_exported(tagged_alias);
        let tagged_alias_type = session.used_by_type(&mut tp, tagged_alias_declared);
        if tagged_alias_type.incomplete
            || tagged_alias_type.minimum != ApiStabilityLevel::Stable
            || !tagged_alias_type.dependencies.is_empty()
        {
            t.fatal(format!(
                "TaggedAliasToConcrete's represented type excludes Concrete's own tag, got min={} deps={}",
                lv(tagged_alias_type.minimum),
                names(&tagged_alias_type)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:237 TestApiStabilityOverloadTagsStayDistinct 70af57d072a1abdc
#[test]
fn test_api_stability_overload_tags_stay_distinct() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r#"/** @stability unstable */
export function mixed(x: string): string
/** @stability experimental */
export function mixed(x: number): number
export function mixed(x: string | number): string | number { return "" }

export function other(x: string): string
/** @stability experimental */
export function other(x: number): number
export function other(x: string | number): string | number { return "" }
"#,
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let mut session = tp.new_api_stability_session();

        let mixed = export_of(t, &mut tp, test_file, "mixed");
        let declared = tp.declared_api_stability_of_symbol(mixed);
        if declared.level != ApiStabilityLevel::Unstable {
            t.fatal(format!(
                "mixed declared = {}, want unstable (the first tagged own declaration)",
                lv(declared.level)
            ));
        }
        let signatures = api_stability_test_signatures(t, tp.checker, Some(mixed));
        if signatures.len() != 2 {
            t.fatal(format!("expected two public overloads, got {}", signatures.len()));
        }
        if tp.declared_api_stability_of_signature(signatures[0]).level
            != ApiStabilityLevel::Unstable
            || tp.declared_api_stability_of_signature(signatures[1]).level
                != ApiStabilityLevel::Experimental
        {
            let first = tp.declared_api_stability_of_signature(signatures[0]).level;
            let second = tp.declared_api_stability_of_signature(signatures[1]).level;
            t.fatal(format!("overload tags collapsed: {}/{}", lv(first), lv(second)));
        }

        let mixed_used = session.used_by_symbol(&mut tp, mixed);
        if mixed_used.incomplete || mixed_used.minimum != ApiStabilityLevel::Experimental {
            t.fatal(format!(
                "mixed surface = {}/{}, want experimental from the other overload",
                lv(mixed_used.minimum),
                names(&mixed_used)
            ));
        }
        let mut found_experimental = false;
        for dependency in &mixed_used.dependencies {
            if dependency.signature.is_some() && dependency.level == ApiStabilityLevel::Experimental
            {
                found_experimental = true;
            }
            if dependency.signature.is_some()
                && raw_signature(dependency.signature) == raw_signature(Some(signatures[0]))
            {
                t.fatal(format!(
                    "the declared own overload leaked into the surface: {}",
                    names(&mixed_used)
                ));
            }
        }
        if !found_experimental {
            t.fatal(format!(
                "the experimental overload should stay an exposed child, got {}",
                names(&mixed_used)
            ));
        }

        for (index, &signature) in signatures.iter().enumerate() {
            let used = session.used_by_signature(&mut tp, signature);
            if used.incomplete
                || used.minimum != ApiStabilityLevel::Stable
                || !used.dependencies.is_empty()
            {
                t.fatal(format!(
                    "overload {index} own tag must be excluded, got min={} deps={}",
                    lv(used.minimum),
                    names(&used)
                ));
            }
        }

        let other = export_of(t, &mut tp, test_file, "other");
        let declared = tp.declared_api_stability_of_symbol(other);
        if declared.level != ApiStabilityLevel::Experimental {
            t.fatal(format!("other declared = {}, want experimental", lv(declared.level)));
        }
        let other_used = session.used_by_symbol(&mut tp, other);
        if other_used.incomplete
            || other_used.minimum != ApiStabilityLevel::Stable
            || !other_used.dependencies.is_empty()
        {
            t.fatal(format!(
                "other's only tag is its own declared tag, got min={} deps={}",
                lv(other_used.minimum),
                names(&other_used)
            ));
        }
    });
}

/// Rust-only: the repeated assertion of the boundary tests: a complete result whose minimum is `minimum` and that
/// exposes `dependency` (Go writes the condition out at every call site with its own message).
fn require_exposes(
    t: &T,
    used: &ApiStabilityUsed,
    minimum: ApiStabilityLevel,
    dependency: &str,
    message: &str,
) {
    if used.incomplete || used.minimum != minimum || !contains_dependency(used, dependency) {
        t.fatal(format!("{message}, got min={} deps={}", lv(used.minimum), names(used)));
    }
}

/// Rust-only: Go's `used.incomplete || len(used.dependencies) != 0` failure (a complete, quiet result).
fn require_quiet(t: &T, used: &ApiStabilityUsed, message: &str) {
    if used.incomplete || !used.dependencies.is_empty() {
        t.fatal(format!("{message}, got min={} deps={}", lv(used.minimum), names(used)));
    }
}

// go test: internal/typeparser/api_stability_surface_test.go:309 TestApiStabilityExplicitStableBoundaries 37a8c9dfb16ab14b
#[test]
fn test_api_stability_explicit_stable_boundaries() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }

/** @stability stable */
export interface StableBox<T> { value: T }
export interface PlainBox<T> { value: T }
export interface RootWithStableBox { box: StableBox<E> }
export interface RootWithPlainBox { box: PlainBox<E> }

/** @stability stable */
export interface StableHidden { value: E }
export interface PlainHidden { value: E }
export interface RootWithStableHidden { box: StableHidden }
export interface RootWithPlainHidden { box: PlainHidden }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let mut session = tp.new_api_stability_session();

        let stable_box = export_of(t, &mut tp, test_file, "StableBox");
        let stable_declared = tp.declared_api_stability_of_symbol(stable_box);
        if stable_declared.level != ApiStabilityLevel::Stable
            || stable_declared.declaration.is_none()
        {
            t.fatal(format!("explicit stable child lost its provenance: {stable_declared:?}"));
        }
        let plain_box = export_of(t, &mut tp, test_file, "PlainBox");
        let plain_declared = tp.declared_api_stability_of_symbol(plain_box);
        if plain_declared.level != ApiStabilityLevel::Stable || plain_declared.declaration.is_some()
        {
            t.fatal(format!("absent tag should be stable without provenance: {plain_declared:?}"));
        }

        let stable_box_root =
            used_by_name(t, &mut session, &mut tp, test_file, "RootWithStableBox");
        require_exposes(
            t,
            &stable_box_root,
            ApiStabilityLevel::Experimental,
            "E",
            "RootWithStableBox should expose the StableBox type argument E",
        );
        let plain_box_root = used_by_name(t, &mut session, &mut tp, test_file, "RootWithPlainBox");
        require_exposes(
            t,
            &plain_box_root,
            ApiStabilityLevel::Experimental,
            "E",
            "RootWithPlainBox should expose the PlainBox type argument E",
        );

        let stable_hidden = used_by_name(t, &mut session, &mut tp, test_file, "StableHidden");
        require_exposes(
            t,
            &stable_hidden,
            ApiStabilityLevel::Experimental,
            "E",
            "StableHidden as a root should expand and expose E",
        );

        let stable_hidden_root =
            used_by_name(t, &mut session, &mut tp, test_file, "RootWithStableHidden");
        require_quiet(
            t,
            &stable_hidden_root,
            "RootWithStableHidden must not expand the nested component",
        );
        let plain_hidden_root =
            used_by_name(t, &mut session, &mut tp, test_file, "RootWithPlainHidden");
        require_quiet(
            t,
            &plain_hidden_root,
            "RootWithPlainHidden must not expand the nested component",
        );
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:370 TestApiStabilityChildrenStillChecked 4cc4ef637558119c
#[test]
fn test_api_stability_children_still_checked() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
interface E { value: string }
declare function make<T>(): { nested: { fn: (x: T) => T } }

/** @stability unstable */
export const taggedInferred = make<E>()
export const cleanInferred = make<string>()

/** @stability unstable */
export declare function taggedGeneric<T extends E>(x: T): T

/** @stability unstable */
export interface TaggedConstrained<T extends E> { value: T }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let mut session = tp.new_api_stability_session();

        let inferred = used_by_name(t, &mut session, &mut tp, test_file, "taggedInferred");
        require_exposes(
            t,
            &inferred,
            ApiStabilityLevel::Experimental,
            "E",
            "an inferred nested anonymous return should still expose E",
        );
        assert_api_stability_surface_excludes_own_tag(t, &inferred, "taggedInferred");

        let clean = used_by_name(t, &mut session, &mut tp, test_file, "cleanInferred");
        require_quiet(t, &clean, "cleanInferred must stay clean");

        let generic = export_of(t, &mut tp, test_file, "taggedGeneric");
        let generic_used = session.used_by_symbol(&mut tp, generic);
        require_exposes(
            t,
            &generic_used,
            ApiStabilityLevel::Experimental,
            "E",
            "a tagged generic function should still expose its constraint",
        );
        assert_api_stability_surface_excludes_own_tag(t, &generic_used, "taggedGeneric");
        let signature = api_stability_test_signatures(t, tp.checker, Some(generic))[0];
        let signature_used = session.used_by_signature(&mut tp, signature);
        require_exposes(
            t,
            &signature_used,
            ApiStabilityLevel::Experimental,
            "E",
            "a tagged signature should still expose its constraint",
        );
        assert_api_stability_surface_excludes_own_tag(t, &signature_used, "taggedGeneric");

        let constrained = used_by_name(t, &mut session, &mut tp, test_file, "TaggedConstrained");
        require_exposes(
            t,
            &constrained,
            ApiStabilityLevel::Experimental,
            "E",
            "a tagged generic interface should still expose its constraint",
        );
        assert_api_stability_surface_excludes_own_tag(t, &constrained, "TaggedConstrained");
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:425 TestApiStabilityChildBoundaryDifferentials 19eb787109e7e764
#[test]
fn test_api_stability_child_boundary_differentials() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }

/** @stability stable */
export interface TaggedCallable { (): E }
export interface PlainCallable { (): E }
export interface CallableHolder { tagged: TaggedCallable; plain: PlainCallable }

/** @stability unstable */
export interface UnstableCallable { (): E }
export interface UnstableCallableHolder { callable: UnstableCallable }

export interface AnonymousWrapper { inner: { callable: TaggedCallable } }

/** @stability stable */
export interface TaggedHidden { secret: E }
export interface PlainHidden { secret: E }
export interface HiddenHolder { tagged: TaggedHidden; plain: PlainHidden }

/** @stability stable */
export interface TaggedBase { secret: E }
export interface PlainBase { secret: E }
export interface TaggedHeritage extends TaggedBase {}
export interface PlainHeritage extends PlainBase {}

/** @stability unstable */
export interface UnstableBase { secret: E }
export interface UnstableHeritage extends UnstableBase {}

/** @stability stable */
export interface TaggedBox<T> { value: T; extra: E }
export interface TaggedBoxStableArgument extends TaggedBox<string> {}
export interface TaggedBoxExposedArgument extends TaggedBox<E> {}

/** @stability stable */
export interface TaggedChain<T> { value: T }
export interface UntaggedMid<U> extends TaggedChain<U> {}
export interface DeepChain extends UntaggedMid<E> {}
export interface DeepChainRef<T> extends UntaggedMid<T> {}
export declare const deepChainRef: DeepChainRef<E>

/** @stability stable */
export type TaggedAliasBox<T> = { value: T }
export interface TaggedAliasBoxHolder { box: TaggedAliasBox<E> }

export interface MemberTagBoundary {
  /** @stability stable */
  anonymous: { inner: E }
  /** @stability stable */
  (): E
}
export interface MemberTagControl {
  anonymous: { inner: E }
  (): E
}

export interface IndexTagBoundary {
  /** @stability stable */
  [key: string]: E
}
export interface IndexTagControl {
  [key: string]: E
}

/** @stability unstable */
export interface SharedBoundary { secret: E }
export interface SharedBoundaryHolder { child: SharedBoundary }

export interface HeritageOverride extends TaggedBase {
  secret: E
}

/** @stability stable */
export interface TaggedIndexedBox<T> { value: T }
export interface BlockedHeritage<T, K extends keyof T> extends TaggedIndexedBox<T[K]> {}
export interface BlockedMember<T, K extends keyof T> { box: TaggedIndexedBox<T[K]> }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let mut session = tp.new_api_stability_session();
        let mut surface = |tp: &mut TypeParser<'_>, name: &str| {
            t.helper();
            used_by_name(t, &mut session, tp, test_file, name)
        };
        let experimental = ApiStabilityLevel::Experimental;
        let unstable = ApiStabilityLevel::Unstable;

        let callable_holder = surface(&mut tp, "CallableHolder");
        require_exposes(
            t,
            &callable_holder,
            experimental,
            "E",
            "CallableHolder should expose E through its untagged callable",
        );

        let anonymous_wrapper = surface(&mut tp, "AnonymousWrapper");
        if anonymous_wrapper.incomplete || contains_dependency(&anonymous_wrapper, "E") {
            t.fatal(format!(
                "a tagged callable must stay bounded through an anonymous wrapper, got min={} deps={}",
                lv(anonymous_wrapper.minimum),
                names(&anonymous_wrapper)
            ));
        }

        let hidden_holder = surface(&mut tp, "HiddenHolder");
        require_quiet(
            t,
            &hidden_holder,
            "a tagged hidden member and an untagged shallow member must both stay quiet",
        );

        let tagged_hidden = surface(&mut tp, "TaggedHidden");
        require_exposes(
            t,
            &tagged_hidden,
            experimental,
            "E",
            "TaggedHidden as a root should expand and expose E",
        );

        let unstable_callable = surface(&mut tp, "UnstableCallableHolder");
        if unstable_callable.incomplete
            || unstable_callable.minimum != unstable
            || contains_dependency(&unstable_callable, "E")
        {
            t.fatal(format!(
                "UnstableCallableHolder should expose only the unstable child tag, got min={} deps={}",
                lv(unstable_callable.minimum),
                names(&unstable_callable)
            ));
        }
        if !contains_dependency(&unstable_callable, "UnstableCallable") {
            t.fatal(format!(
                "UnstableCallableHolder should expose UnstableCallable, got {}",
                names(&unstable_callable)
            ));
        }
        let unstable_callable_root = surface(&mut tp, "UnstableCallable");
        require_exposes(
            t,
            &unstable_callable_root,
            experimental,
            "E",
            "UnstableCallable as a root should expand and expose E",
        );

        let tagged_heritage = surface(&mut tp, "TaggedHeritage");
        require_quiet(
            t,
            &tagged_heritage,
            "TaggedHeritage must not expose the tagged base internals",
        );
        let plain_heritage = surface(&mut tp, "PlainHeritage");
        require_exposes(
            t,
            &plain_heritage,
            experimental,
            "E",
            "PlainHeritage should expand its untagged base and expose E",
        );
        let unstable_heritage = surface(&mut tp, "UnstableHeritage");
        if unstable_heritage.incomplete
            || unstable_heritage.minimum != unstable
            || contains_dependency(&unstable_heritage, "E")
        {
            t.fatal(format!(
                "UnstableHeritage should expose only the unstable base tag, got min={} deps={}",
                lv(unstable_heritage.minimum),
                names(&unstable_heritage)
            ));
        }
        if !contains_dependency(&unstable_heritage, "UnstableBase") {
            t.fatal(format!(
                "UnstableHeritage should expose UnstableBase, got {}",
                names(&unstable_heritage)
            ));
        }

        let stable_argument = surface(&mut tp, "TaggedBoxStableArgument");
        require_quiet(
            t,
            &stable_argument,
            "TaggedBox<string> must not expose the tagged base internals",
        );
        let exposed_argument = surface(&mut tp, "TaggedBoxExposedArgument");
        require_exposes(
            t,
            &exposed_argument,
            experimental,
            "E",
            "TaggedBox<E> must keep the independently represented argument exposed",
        );

        let deep_chain = surface(&mut tp, "DeepChain");
        require_exposes(
            t,
            &deep_chain,
            experimental,
            "E",
            "DeepChain should expose the tagged ancestor argument through the untagged intermediate",
        );

        let deep_ref_ident = find_identifier_by_text(t, test_file, "DeepChainRef", 1);
        let Some(deep_ref_parent) = deep_ref_ident.parent() else {
            t.fatal("DeepChainRef annotation not found");
        };
        let deep_ref_type = tp.get_type_at_location(deep_ref_parent);
        let deep_ref = session.used_by_type(&mut tp, deep_ref_type);
        require_exposes(
            t,
            &deep_ref,
            experimental,
            "E",
            "the DeepChainRef<E> reference root should expose E",
        );
        let mut surface = |tp: &mut TypeParser<'_>, name: &str| {
            t.helper();
            used_by_name(t, &mut session, tp, test_file, name)
        };

        let alias_box = surface(&mut tp, "TaggedAliasBoxHolder");
        require_exposes(
            t,
            &alias_box,
            experimental,
            "E",
            "a tagged alias application must keep its argument exposed",
        );

        let member_tag_boundary = surface(&mut tp, "MemberTagBoundary");
        require_quiet(
            t,
            &member_tag_boundary,
            "tagged member and tagged call signature must both stay bounded",
        );
        let member_tag_control = surface(&mut tp, "MemberTagControl");
        require_exposes(
            t,
            &member_tag_control,
            experimental,
            "E",
            "MemberTagControl should keep the anonymous walk and expose E",
        );

        let index_tag_boundary = surface(&mut tp, "IndexTagBoundary");
        if index_tag_boundary.incomplete || contains_dependency(&index_tag_boundary, "E") {
            t.fatal(format!(
                "a tagged index signature must not expose its value internals, got min={} deps={}",
                lv(index_tag_boundary.minimum),
                names(&index_tag_boundary)
            ));
        }
        let index_tag_control = surface(&mut tp, "IndexTagControl");
        require_exposes(
            t,
            &index_tag_control,
            experimental,
            "E",
            "IndexTagControl should expose the index value type",
        );

        let shared_holder = surface(&mut tp, "SharedBoundaryHolder");
        if shared_holder.incomplete
            || shared_holder.minimum != unstable
            || contains_dependency(&shared_holder, "E")
        {
            t.fatal(format!(
                "SharedBoundaryHolder should expose only the child tag, got min={} deps={}",
                lv(shared_holder.minimum),
                names(&shared_holder)
            ));
        }
        if !contains_dependency(&shared_holder, "SharedBoundary") {
            t.fatal(format!(
                "SharedBoundaryHolder should expose SharedBoundary, got {}",
                names(&shared_holder)
            ));
        }
        let shared_root = surface(&mut tp, "SharedBoundary");
        require_exposes(
            t,
            &shared_root,
            experimental,
            "E",
            "SharedBoundary as a root should still expand and expose E",
        );

        let heritage_override = surface(&mut tp, "HeritageOverride");
        require_exposes(
            t,
            &heritage_override,
            experimental,
            "E",
            "an own override must not be suppressed by a tagged base of the same name",
        );

        let blocked_heritage = surface(&mut tp, "BlockedHeritage");
        if !blocked_heritage.incomplete {
            t.fatal(format!(
                "a tagged base whose required argument cannot be established must stay incomplete, got min={} deps={}",
                lv(blocked_heritage.minimum),
                names(&blocked_heritage)
            ));
        }
        let blocked_member = surface(&mut tp, "BlockedMember");
        if !blocked_member.incomplete {
            t.fatal(format!(
                "a tagged member whose required argument cannot be established must stay incomplete, got min={} deps={}",
                lv(blocked_member.minimum),
                names(&blocked_member)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:639 TestApiStabilityFlattenedInheritanceBoundaries c7cea5eb4e7a4d1b
#[test]
fn test_api_stability_flattened_inheritance_boundaries() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }
/** @stability experimental */
export interface FxCallE { value: string }
/** @stability experimental */
export interface FxCtorE { value: string }
/** @stability experimental */
export interface FxIdxE { value: string }
/** @stability experimental */
export interface FxMethE { value: string }
/** @stability experimental */
export interface FxPropE { value: string }
/** @stability experimental */
export interface FxArgE { value: string }

/** @stability unstable */
export interface FxTaggedCallableBase { (): FxCallE }
export interface FxDerivedCallable extends FxTaggedCallableBase {}
export interface FxDerivedCallableGeneric<T> extends FxTaggedCallableBase { own: T }
export declare const fxCallRef: FxDerivedCallableGeneric<string>
interface FxPlainCallableBase { (): FxCallE }
export interface FxPlainDerivedCallableGeneric<T> extends FxPlainCallableBase { own: T }
export declare const fxPlainCallRef: FxPlainDerivedCallableGeneric<string>

/** @stability unstable */
export interface FxTaggedCombo {
  (): FxCallE
  new (): FxCtorE
  [key: string]: FxIdxE
  m(): FxMethE
  p: FxPropE
}
export interface FxDerivedCombo<T> extends FxTaggedCombo { own: T }
export declare const fxComboRef: FxDerivedCombo<string>

/** @stability unstable */
export interface FxTaggedIndexBase { [key: string]: FxIdxE }
export interface FxDerivedIndex<T> extends FxTaggedIndexBase { own: T }
export declare const fxIndexRef: FxDerivedIndex<string>
interface FxPlainIndexBase { [key: string]: FxIdxE }
export interface FxPlainDerivedIndex<T> extends FxPlainIndexBase { own: T }
export declare const fxPlainIndexRef: FxPlainDerivedIndex<string>

/** @stability unstable */
export interface FxTaggedDeepCallable { (): FxCallE }
export interface FxMidCallable<T> extends FxTaggedDeepCallable { mid: T }
export interface FxDeepCallable<T> extends FxMidCallable<T> {}
export declare const fxDeepCallRef: FxDeepCallable<string>

/** @stability unstable */
export class FxTaggedClassBase {
  constructor(x: FxCtorE)
  static s: FxPropE
  m(): FxMethE
}
export class FxDerivedClass extends FxTaggedClassBase {}
export class FxDerivedOwnCtorClass extends FxTaggedClassBase {
  constructor(y: E) { super(y as unknown as FxCtorE) }
}
/** @stability unstable */
export class FxTaggedNoCtorClass { m(): FxMethE }
export class FxDerivedNoCtorClass extends FxTaggedNoCtorClass {}
class FxPlainClassBase {
  constructor(x: FxCtorE)
  static s: FxPropE
  m(): FxMethE
}
export class FxPlainDerivedClass extends FxPlainClassBase {}

/** @stability stable */
export interface FxStableCallableBase { (): FxCallE }
export interface FxDerivedStableCallable extends FxStableCallableBase {}

/** @stability unstable */
export interface FxOwnOverrideBase { (): FxCallE; [key: string]: FxIdxE }
export interface FxOwnOverrideDerived extends FxOwnOverrideBase {
  (): E
  [key: string]: E
}

/** @stability unstable */
export interface FxTaggedCallableBox<T> { (): FxPropE; value: T }
export interface FxDerivedCallableBox<T> extends FxTaggedCallableBox<T> {}
export declare const fxCallableBoxRef: FxDerivedCallableBox<FxArgE>

/** @stability unstable */
export interface FxTaggedFirstCall { (): FxCallE }
interface FxPlainSecond { own: string }
export interface FxMixedCallInheritance extends FxTaggedFirstCall, FxPlainSecond {}

/** @stability unstable */
export interface FxTaggedBlockedBox<T> { value: T }
export interface FxBlockedDerived<T, K extends keyof T> extends FxTaggedBlockedBox<T[K]> {}
",
            )],
        );
        let test_file = files["/.src/test.ts"];
        let assert_quiet = |label: &str, used: &ApiStabilityUsed, dependency: &str| {
            t.helper();
            if contains_dependency(used, dependency) {
                t.fatal(format!(
                    "{label} must not expose {dependency}, got incomplete={} min={} deps={}",
                    used.incomplete,
                    lv(used.minimum),
                    names(used)
                ));
            }
        };
        let assert_exposed = |label: &str, used: &ApiStabilityUsed, dependency: &str| {
            t.helper();
            if used.incomplete || !contains_dependency(used, dependency) {
                t.fatal(format!(
                    "{label} should expose {dependency}, got incomplete={} min={} deps={}",
                    used.incomplete,
                    lv(used.minimum),
                    names(used)
                ));
            }
        };
        let internals = ["FxCallE", "FxCtorE", "FxIdxE", "FxMethE", "FxPropE"];

        let mut cold = {
            let mut tp = compiled.type_parser();
            let mut cold = tp.new_api_stability_session();
            let type_of = |tp: &mut TypeParser<'_>, name: &str| {
                let symbol = export_of(t, tp, test_file, name);
                tp.checker.get_type_of_symbol_exported(symbol)
            };
            // Rust: tsrs's GetTypeOfSymbol never returns nil, so Go's "cold reference types must be
            // representable" check cannot fail.
            let cold_call_ref_type = type_of(&mut tp, "fxCallRef");
            let cold_index_ref_type = type_of(&mut tp, "fxIndexRef");
            let cold_combo_ref_type = type_of(&mut tp, "fxComboRef");
            let cold_deep_ref_type = type_of(&mut tp, "fxDeepCallRef");
            let cold_derived_class_static = type_of(&mut tp, "FxDerivedClass");
            let tests: Vec<(&str, ApiStabilityUsed)> = vec![
                (
                    "cold FxDerivedCallable declaration root",
                    used_by_name(t, &mut cold, &mut tp, test_file, "FxDerivedCallable"),
                ),
                (
                    "cold FxDerivedCallableGeneric declaration root",
                    used_by_name(t, &mut cold, &mut tp, test_file, "FxDerivedCallableGeneric"),
                ),
                (
                    "cold FxDerivedIndex declaration root",
                    used_by_name(t, &mut cold, &mut tp, test_file, "FxDerivedIndex"),
                ),
                (
                    "cold FxDerivedClass declaration root",
                    used_by_name(t, &mut cold, &mut tp, test_file, "FxDerivedClass"),
                ),
                ("cold fxCallRef reference root", cold.used_by_type(&mut tp, cold_call_ref_type)),
                ("cold fxIndexRef reference root", cold.used_by_type(&mut tp, cold_index_ref_type)),
                ("cold fxComboRef reference root", cold.used_by_type(&mut tp, cold_combo_ref_type)),
                (
                    "cold fxDeepCallRef reference root",
                    cold.used_by_type(&mut tp, cold_deep_ref_type),
                ),
                (
                    "cold fxDerivedClass static type root",
                    cold.used_by_type(&mut tp, cold_derived_class_static),
                ),
            ];
            for (label, used) in &tests {
                for dependency in internals {
                    assert_quiet(label, used, dependency);
                }
            }
            cold
        };

        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();
        let mut warm = tp.new_api_stability_session();
        let unstable = ApiStabilityLevel::Unstable;

        // Rust: Go's `surface` and `referenceSurface` closures both borrow the warm session and the parser,
        // so they are macros here.
        macro_rules! reference_surface {
            ($name:expr) => {{
                let symbol = export_of(t, &mut tp, test_file, $name);
                let represented = tp.checker.get_type_of_symbol_exported(symbol);
                warm.used_by_type(&mut tp, represented)
            }};
        }
        macro_rules! surface {
            ($name:expr) => {
                used_by_name(t, &mut warm, &mut tp, test_file, $name)
            };
        }

        let derived_callable = surface!("FxDerivedCallable");
        if derived_callable.incomplete
            || derived_callable.minimum != unstable
            || contains_dependency(&derived_callable, "FxCallE")
        {
            t.fatal(format!(
                "FxDerivedCallable must expose only the tagged base, got min={} deps={}",
                lv(derived_callable.minimum),
                names(&derived_callable)
            ));
        }
        if !contains_dependency(&derived_callable, "FxTaggedCallableBase") {
            t.fatal(format!(
                "FxDerivedCallable should expose FxTaggedCallableBase, got {}",
                names(&derived_callable)
            ));
        }
        let derived_callable_generic = surface!("FxDerivedCallableGeneric");
        if derived_callable_generic.incomplete
            || derived_callable_generic.minimum != unstable
            || contains_dependency(&derived_callable_generic, "FxCallE")
        {
            t.fatal(format!(
                "FxDerivedCallableGeneric must expose only the tagged base, got min={} deps={}",
                lv(derived_callable_generic.minimum),
                names(&derived_callable_generic)
            ));
        }
        let call_ref_type = reference_surface!("fxCallRef");
        if call_ref_type.incomplete
            || call_ref_type.minimum != unstable
            || contains_dependency(&call_ref_type, "FxCallE")
        {
            t.fatal(format!(
                "fxCallRef type root must expose only the tagged base, got min={} deps={}",
                lv(call_ref_type.minimum),
                names(&call_ref_type)
            ));
        }
        if !contains_dependency(&call_ref_type, "FxTaggedCallableBase") {
            t.fatal(format!(
                "fxCallRef type root should expose FxTaggedCallableBase, got {}",
                names(&call_ref_type)
            ));
        }
        let call_ref_symbol = surface!("fxCallRef");
        if call_ref_symbol.incomplete
            || call_ref_symbol.minimum != unstable
            || contains_dependency(&call_ref_symbol, "FxCallE")
        {
            t.fatal(format!(
                "fxCallRef value root must expose only the tagged base, got min={} deps={}",
                lv(call_ref_symbol.minimum),
                names(&call_ref_symbol)
            ));
        }
        assert_exposed(
            "fxPlainCallRef untagged control",
            &reference_surface!("fxPlainCallRef"),
            "FxCallE",
        );

        for label in ["combo type root", "combo value root"] {
            let used = if label == "combo type root" {
                reference_surface!("fxComboRef")
            } else {
                surface!("fxComboRef")
            };
            if used.incomplete
                || used.minimum != unstable
                || !contains_dependency(&used, "FxTaggedCombo")
            {
                t.fatal(format!(
                    "{label} must expose only FxTaggedCombo, got min={} deps={}",
                    lv(used.minimum),
                    names(&used)
                ));
            }
            for dependency in internals {
                assert_quiet(label, &used, dependency);
            }
        }

        let derived_index = surface!("FxDerivedIndex");
        if derived_index.incomplete
            || derived_index.minimum != unstable
            || contains_dependency(&derived_index, "FxIdxE")
        {
            t.fatal(format!(
                "FxDerivedIndex must expose only the tagged base, got min={} deps={}",
                lv(derived_index.minimum),
                names(&derived_index)
            ));
        }
        let index_ref_type = reference_surface!("fxIndexRef");
        if index_ref_type.incomplete
            || index_ref_type.minimum != unstable
            || contains_dependency(&index_ref_type, "FxIdxE")
        {
            t.fatal(format!(
                "fxIndexRef type root must expose only the tagged base, got min={} deps={}",
                lv(index_ref_type.minimum),
                names(&index_ref_type)
            ));
        }
        assert_quiet("fxIndexRef value root", &surface!("fxIndexRef"), "FxIdxE");
        assert_exposed(
            "fxPlainIndexRef untagged control",
            &reference_surface!("fxPlainIndexRef"),
            "FxIdxE",
        );

        let deep_call_ref = reference_surface!("fxDeepCallRef");
        if deep_call_ref.incomplete || contains_dependency(&deep_call_ref, "FxCallE") {
            t.fatal(format!(
                "fxDeepCallRef must not expose the tagged ancestor's call internals, got min={} deps={}",
                lv(deep_call_ref.minimum),
                names(&deep_call_ref)
            ));
        }
        if !contains_dependency(&deep_call_ref, "FxTaggedDeepCallable") {
            t.fatal(format!(
                "fxDeepCallRef should expose FxTaggedDeepCallable, got {}",
                names(&deep_call_ref)
            ));
        }

        let derived_class = surface!("FxDerivedClass");
        if derived_class.incomplete || derived_class.minimum != unstable {
            t.fatal(format!(
                "FxDerivedClass must expose the tagged base level, got min={} deps={}",
                lv(derived_class.minimum),
                names(&derived_class)
            ));
        }
        for dependency in ["FxCtorE", "FxMethE", "FxPropE"] {
            assert_quiet("FxDerivedClass", &derived_class, dependency);
        }
        let derived_class_static = reference_surface!("FxDerivedClass");
        assert_quiet("FxDerivedClass static type root", &derived_class_static, "FxCtorE");
        assert_exposed(
            "FxPlainDerivedClass untagged control",
            &surface!("FxPlainDerivedClass"),
            "FxCtorE",
        );

        let own_ctor = surface!("FxDerivedOwnCtorClass");
        assert_exposed("FxDerivedOwnCtorClass own constructor", &own_ctor, "E");
        assert_quiet("FxDerivedOwnCtorClass", &own_ctor, "FxCtorE");

        let derived_no_ctor = surface!("FxDerivedNoCtorClass");
        if derived_no_ctor.incomplete
            || derived_no_ctor.minimum != unstable
            || !contains_dependency(&derived_no_ctor, "FxTaggedNoCtorClass")
        {
            t.fatal(format!(
                "FxDerivedNoCtorClass must expose only the tagged base level, got incomplete={} min={} deps={}",
                derived_no_ctor.incomplete,
                lv(derived_no_ctor.minimum),
                names(&derived_no_ctor)
            ));
        }
        assert_quiet("FxDerivedNoCtorClass", &derived_no_ctor, "FxMethE");
        assert_quiet(
            "FxDerivedNoCtorClass static type root",
            &reference_surface!("FxDerivedNoCtorClass"),
            "FxMethE",
        );
        let cold_no_ctor = used_by_name(t, &mut cold, &mut tp, test_file, "FxDerivedNoCtorClass");
        assert_quiet("cold FxDerivedNoCtorClass", &cold_no_ctor, "FxMethE");

        let stable_callable = surface!("FxDerivedStableCallable");
        require_quiet(t, &stable_callable, "FxDerivedStableCallable must stay complete and quiet");

        let own_override = surface!("FxOwnOverrideDerived");
        assert_exposed("FxOwnOverrideDerived own signatures", &own_override, "E");
        assert_quiet("FxOwnOverrideDerived", &own_override, "FxCallE");
        assert_quiet("FxOwnOverrideDerived", &own_override, "FxIdxE");

        let callable_box_ref = reference_surface!("fxCallableBoxRef");
        assert_exposed("fxCallableBoxRef tagged ancestor argument", &callable_box_ref, "FxArgE");
        assert_quiet("fxCallableBoxRef", &callable_box_ref, "FxPropE");
        if !contains_dependency(&callable_box_ref, "FxTaggedCallableBox") {
            t.fatal(format!(
                "fxCallableBoxRef should expose FxTaggedCallableBox, got {}",
                names(&callable_box_ref)
            ));
        }

        let mixed_call = surface!("FxMixedCallInheritance");
        assert_quiet("FxMixedCallInheritance", &mixed_call, "FxCallE");
        if !contains_dependency(&mixed_call, "FxTaggedFirstCall") {
            t.fatal(format!(
                "FxMixedCallInheritance should expose FxTaggedFirstCall, got {}",
                names(&mixed_call)
            ));
        }

        let blocked = surface!("FxBlockedDerived");
        if !blocked.incomplete {
            t.fatal(format!(
                "a tagged base with a blocked required argument must stay incomplete, got min={} deps={}",
                lv(blocked.minimum),
                names(&blocked)
            ));
        }

        let callable_base_root = surface!("FxTaggedCallableBase");
        assert_exposed("FxTaggedCallableBase root", &callable_base_root, "FxCallE");
        let combo_base_root = surface!("FxTaggedCombo");
        for dependency in internals {
            assert_exposed("FxTaggedCombo root", &combo_base_root, dependency);
        }
        let class_base_root = surface!("FxTaggedClassBase");
        for dependency in ["FxCtorE", "FxMethE", "FxPropE"] {
            assert_exposed("FxTaggedClassBase root", &class_base_root, dependency);
        }
        let index_base_root = surface!("FxTaggedIndexBase");
        assert_exposed("FxTaggedIndexBase root", &index_base_root, "FxIdxE");

        let globals = tp.checker.get_global_diagnostics();
        if !globals.is_empty() {
            t.error(format!(
                "flattened inheritance queries produced global diagnostics: {} diagnostics",
                globals.len()
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:923 apiStabilitySharedTypeEntry d9ef2a0d553da373
// Rust: the snapshot is returned as a copy (the links are behind a lock), so Go's snapshot pointer identity
// checks become content comparisons.
pub(crate) fn api_stability_shared_type_entry(
    tp: &TypeParser<'_>,
    target: Option<P<Type>>,
    inspect: ApiStabilityInspection,
) -> Option<ApiStabilitySharedSurface> {
    let t = target?;
    tp.links().api_stability_surface_type.get(&ApiStabilitySurfaceTypeKey { t, inspect }).cloned()
}

// go test: internal/typeparser/api_stability_surface_test.go:930 apiStabilitySharedSignatureEntry 01515619309090e5
pub(crate) fn api_stability_shared_signature_entry(
    tp: &TypeParser<'_>,
    signature: Option<P<Signature>>,
) -> Option<ApiStabilitySharedSurface> {
    let signature = signature?;
    tp.links().api_stability_surface_signature.get(&signature).cloned()
}

// go test: internal/typeparser/api_stability_surface_test.go:937 apiStabilitySharedEntryHasSymbol 7edf6a5abb4a0d76
pub(crate) fn api_stability_shared_entry_has_symbol(
    entry: Option<&ApiStabilitySharedSurface>,
    symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(entry), Some(symbol)) = (entry, symbol) else {
        return false;
    };
    entry
        .findings
        .contains_key(&ApiStabilityFindingKey { symbol: Some(symbol), ..Default::default() })
}

// go test: internal/typeparser/api_stability_surface_test.go:945 apiStabilitySharedEntryRemovedSymbol 4e96468441d3ce94
pub(crate) fn api_stability_shared_entry_removed_symbol(
    entry: Option<&ApiStabilitySharedSurface>,
    symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(entry), Some(symbol)) = (entry, symbol) else {
        return false;
    };
    entry.removed.iter().any(|key| key.symbol == Some(symbol))
}

// go test: internal/typeparser/api_stability_surface_test.go:957 apiStabilitySharedEntryHasSignature 3b2ba13559a88ff4
pub(crate) fn api_stability_shared_entry_has_signature(
    entry: Option<&ApiStabilitySharedSurface>,
    signature: Option<P<Signature>>,
) -> bool {
    let (Some(entry), Some(signature)) = (entry, signature) else {
        return false;
    };
    let raw = raw_signature(Some(signature));
    entry.findings.keys().any(|key| key.signature.is_some() && raw_signature(key.signature) == raw)
}

// go test: internal/typeparser/api_stability_surface_test.go:970 apiStabilitySharedEntryRemovedSignature 2ca65e214aab8ab3
pub(crate) fn api_stability_shared_entry_removed_signature(
    entry: Option<&ApiStabilitySharedSurface>,
    signature: Option<P<Signature>>,
) -> bool {
    let (Some(entry), Some(signature)) = (entry, signature) else {
        return false;
    };
    let raw = raw_signature(Some(signature));
    entry.removed.iter().any(|key| key.signature.is_some() && raw_signature(key.signature) == raw)
}

// go test: internal/typeparser/api_stability_surface_test.go:983 TestApiStabilitySnapshotsExcludeOwnTagAndCompose 788f709bf75cda47
#[test]
fn test_api_stability_snapshots_exclude_own_tag_and_compose() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Tagged { leak: E; plain: number }
/** @stability unstable */
export declare function taggedFn(): E
export interface Holder { tagged: Tagged; fn: typeof taggedFn }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let e_symbol = export_of(t, &mut tp, test_file, "E");
        let tagged = export_of(t, &mut tp, test_file, "Tagged");
        let tagged_decl = tp.checker.get_declared_type_of_symbol_exported(tagged);
        let tagged_fn = export_of(t, &mut tp, test_file, "taggedFn");
        let signature = api_stability_test_signatures(t, tp.checker, Some(tagged_fn))[0];

        let root_type = tp.new_api_stability_session().used_by_type(&mut tp, tagged_decl);
        if root_type.incomplete
            || root_type.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&root_type, "E")
        {
            t.fatal(format!(
                "Tagged type root = {}/{}, want experimental E",
                lv(root_type.minimum),
                names(&root_type)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &root_type, "Tagged");
        let type_entry =
            api_stability_shared_type_entry(&tp, Some(tagged_decl), ApiStabilityInspection::Expand);
        if type_entry.is_none() {
            t.fatal("a complete root type surface must be published");
        }
        if api_stability_shared_entry_has_symbol(type_entry.as_ref(), Some(tagged)) {
            t.fatal("the published snapshot must not keep the component's own tag finding");
        }
        if !api_stability_shared_entry_removed_symbol(type_entry.as_ref(), Some(tagged)) {
            t.fatal("the published snapshot must record the removed own-tag finding");
        }
        if !api_stability_shared_entry_has_symbol(type_entry.as_ref(), Some(e_symbol)) {
            t.fatal("the published snapshot must keep the exposed child findings");
        }

        let root_signature = tp.new_api_stability_session().used_by_signature(&mut tp, signature);
        if root_signature.incomplete
            || root_signature.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&root_signature, "E")
        {
            t.fatal(format!(
                "taggedFn signature root = {}/{}, want experimental E",
                lv(root_signature.minimum),
                names(&root_signature)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &root_signature, "taggedFn");
        let signature_entry = api_stability_shared_signature_entry(&tp, Some(signature));
        if signature_entry.is_none() {
            t.fatal("a complete root signature surface must be published");
        }
        if api_stability_shared_entry_has_signature(signature_entry.as_ref(), Some(signature)) {
            t.fatal("the published signature snapshot must not keep the own tag finding");
        }
        if !api_stability_shared_entry_removed_signature(signature_entry.as_ref(), Some(signature))
        {
            t.fatal("the published signature snapshot must record the removed own-tag finding");
        }

        let reused_type = tp.new_api_stability_session().used_by_type(&mut tp, tagged_decl);
        if reused_type.incomplete
            || reused_type.minimum != ApiStabilityLevel::Experimental
            || reused_type.dependencies.len() != root_type.dependencies.len()
        {
            t.fatal(format!(
                "reused type root disagrees: {}/{}",
                lv(reused_type.minimum),
                names(&reused_type)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &reused_type, "Tagged");
        let reused_signature = tp.new_api_stability_session().used_by_signature(&mut tp, signature);
        if reused_signature.incomplete
            || reused_signature.minimum != ApiStabilityLevel::Experimental
            || reused_signature.dependencies.len() != root_signature.dependencies.len()
        {
            t.fatal(format!(
                "reused signature root disagrees: {}/{}",
                lv(reused_signature.minimum),
                names(&reused_signature)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &reused_signature, "taggedFn");

        let holder = export_of(t, &mut tp, test_file, "Holder");
        let holder_used = tp.new_api_stability_session().used_by_symbol(&mut tp, holder);
        if holder_used.incomplete || holder_used.minimum != ApiStabilityLevel::Unstable {
            t.fatal(format!(
                "Holder = {}/{}, want unstable from its tagged children",
                lv(holder_used.minimum),
                names(&holder_used)
            ));
        }
        if !contains_dependency(&holder_used, "Tagged")
            || !contains_dependency(&holder_used, "taggedFn")
        {
            t.fatal(format!(
                "Holder should expose both tagged children, got {}",
                names(&holder_used)
            ));
        }
        if contains_dependency(&holder_used, "E") {
            t.fatal(format!(
                "a cached tagged child must not leak its internals, got {}",
                names(&holder_used)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1065 TestApiStabilitySnapshotsReuseAcrossExportsFilesAndParsers 18649683032d3320
#[test]
fn test_api_stability_snapshots_reuse_across_exports_files_and_parsers() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                (
                    "/.src/dep.ts",
                    r"/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Shared { leak: E }
",
                ),
                (
                    "/.src/consumer.ts",
                    r#"import { Shared } from "./dep.js"
export interface Consumer { shared: Shared }
export interface AlsoConsumer { other: Shared }
"#,
                ),
            ],
        );
        prime_api_stability_checker(t, &mut compiled);

        let (shared, shared_decl, first, published) = {
            let mut tp1 = compiled.type_parser();
            let shared = export_of(t, &mut tp1, files["/.src/dep.ts"], "Shared");
            let shared_decl = tp1.checker.get_declared_type_of_symbol_exported(shared);

            let first = tp1.new_api_stability_session().used_by_type(&mut tp1, shared_decl);
            if first.incomplete
                || first.minimum != ApiStabilityLevel::Experimental
                || !contains_dependency(&first, "E")
            {
                t.fatal(format!(
                    "Shared root = {}/{}, want experimental E",
                    lv(first.minimum),
                    names(&first)
                ));
            }
            let entry = api_stability_shared_type_entry(
                &tp1,
                Some(shared_decl),
                ApiStabilityInspection::Expand,
            );
            let Some(entry) = entry else {
                t.fatal("the first export must publish the complete Shared surface");
            };
            (shared, shared_decl, first, entry.findings.len())
        };

        let mut tp2 = new_type_parser(compiled.program, &mut compiled.checker);
        let mut hit = new_api_stability_analysis();
        hit.type_surface(
            &mut tp2,
            Some(shared_decl),
            ApiStabilityInspection::Expand,
            &ApiStabilitySubstitution::default(),
        );
        if hit.work != 0 {
            t.fatal(format!("a published snapshot hit consumed {} work", hit.work));
        }
        let second = tp2.new_api_stability_session().used_by_type(&mut tp2, shared_decl);
        if second.minimum != first.minimum || second.dependencies.len() != first.dependencies.len()
        {
            t.fatal(format!(
                "reused root disagrees: first={} second={}",
                names(&first),
                names(&second)
            ));
        }

        let consumer = export_of(t, &mut tp2, files["/.src/consumer.ts"], "Consumer");
        let consumer_used = tp2.new_api_stability_session().used_by_symbol(&mut tp2, consumer);
        if consumer_used.incomplete
            || consumer_used.minimum != ApiStabilityLevel::Unstable
            || !contains_dependency(&consumer_used, "Shared")
        {
            t.fatal(format!(
                "Consumer = {}/{}, want unstable Shared",
                lv(consumer_used.minimum),
                names(&consumer_used)
            ));
        }
        let shallow_entry = api_stability_shared_type_entry(
            &tp2,
            Some(shared_decl),
            ApiStabilityInspection::Shallow,
        );
        if shallow_entry.is_none() {
            t.fatal("the shallow child surface must be published for the same concrete type");
        }
        let also_consumer = export_of(t, &mut tp2, files["/.src/consumer.ts"], "AlsoConsumer");
        let mut shallow_hit = new_api_stability_analysis();
        shallow_hit.type_surface(
            &mut tp2,
            Some(shared_decl),
            ApiStabilityInspection::Shallow,
            &ApiStabilitySubstitution::default(),
        );
        if shallow_hit.work != 0 {
            t.fatal(format!("a published shallow snapshot hit consumed {} work", shallow_hit.work));
        }
        let also_used = tp2.new_api_stability_session().used_by_symbol(&mut tp2, also_consumer);
        if also_used.minimum != consumer_used.minimum
            || also_used.dependencies.len() != consumer_used.dependencies.len()
        {
            t.fatal(format!(
                "second consumer disagrees: {}/{}",
                lv(also_used.minimum),
                names(&also_used)
            ));
        }

        let after = api_stability_shared_type_entry(
            &tp2,
            Some(shared_decl),
            ApiStabilityInspection::Expand,
        );
        if after.as_ref().is_none_or(|after| after.findings.len() != published) {
            t.fatal(format!(
                "the published expand snapshot was mutated: before={published} after={after:?}"
            ));
        }
        if api_stability_shared_entry_has_symbol(after.as_ref(), Some(shared)) {
            t.fatal("the published snapshot gained the own tag after reuse");
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1135 TestApiStabilityInspectionModesStaySeparate 180baa2dcbc9913a
#[test]
fn test_api_stability_inspection_modes_stay_separate() {
    run(|t| {
        t.parallel();
        for order in ["expand-first", "shallow-first"] {
            t.run(order, |t| {
                t.parallel();
                let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                    t,
                    &[(
                        "/.src/test.ts",
                        r"/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Tagged { leak: E; plain: number }
export interface Child { tagged: Tagged }
export interface Plain { leak: E }
export interface PlainChild { plain: Plain }
",
                    )],
                );
                prime_api_stability_checker(t, &mut compiled);
                let test_file = files["/.src/test.ts"];
                let mut tp = compiled.type_parser();
                let e_symbol = export_of(t, &mut tp, test_file, "E");
                let tagged = export_of(t, &mut tp, test_file, "Tagged");
                let tagged_decl = tp.checker.get_declared_type_of_symbol_exported(tagged);
                let child = export_of(t, &mut tp, test_file, "Child");
                let plain = export_of(t, &mut tp, test_file, "Plain");
                let plain_decl = tp.checker.get_declared_type_of_symbol_exported(plain);
                let plain_child = export_of(t, &mut tp, test_file, "PlainChild");

                let check_children = |tp: &mut TypeParser<'_>, label: &str| {
                    t.helper();
                    let child_used = tp.new_api_stability_session().used_by_symbol(tp, child);
                    if child_used.incomplete
                        || child_used.minimum != ApiStabilityLevel::Unstable
                        || contains_dependency(&child_used, "E")
                    {
                        t.fatal(format!(
                            "{label}: Child = {}/{}, want only the unstable child",
                            lv(child_used.minimum),
                            names(&child_used)
                        ));
                    }
                    let plain_used = tp.new_api_stability_session().used_by_symbol(tp, plain_child);
                    if plain_used.incomplete || !plain_used.dependencies.is_empty() {
                        t.fatal(format!(
                            "{label}: PlainChild = {}/{}, want a quiet shallow surface",
                            lv(plain_used.minimum),
                            names(&plain_used)
                        ));
                    }
                };
                let check_roots = |tp: &mut TypeParser<'_>, label: &str| {
                    t.helper();
                    let root = tp.new_api_stability_session().used_by_type(tp, tagged_decl);
                    if root.incomplete
                        || root.minimum != ApiStabilityLevel::Experimental
                        || !contains_dependency(&root, "E")
                    {
                        t.fatal(format!(
                            "{label}: Tagged root = {}/{}, want experimental E",
                            lv(root.minimum),
                            names(&root)
                        ));
                    }
                    assert_api_stability_surface_excludes_own_tag(t, &root, "Tagged");
                    let plain_root = tp.new_api_stability_session().used_by_type(tp, plain_decl);
                    if plain_root.incomplete
                        || plain_root.minimum != ApiStabilityLevel::Experimental
                        || !contains_dependency(&plain_root, "E")
                    {
                        t.fatal(format!(
                            "{label}: Plain root = {}/{}, want experimental E",
                            lv(plain_root.minimum),
                            names(&plain_root)
                        ));
                    }
                };

                if order == "shallow-first" {
                    check_children(&mut tp, "shallow-first");
                }
                check_roots(&mut tp, order);

                let tagged_expand = api_stability_shared_type_entry(
                    &tp,
                    Some(tagged_decl),
                    ApiStabilityInspection::Expand,
                );
                let plain_expand = api_stability_shared_type_entry(
                    &tp,
                    Some(plain_decl),
                    ApiStabilityInspection::Expand,
                );
                if tagged_expand.is_none() || plain_expand.is_none() {
                    t.fatal("both expanded root snapshots must be published");
                }
                if !api_stability_shared_entry_has_symbol(plain_expand.as_ref(), Some(e_symbol)) {
                    t.fatal("the plain expanded snapshot must keep the internals");
                }

                if order == "expand-first" {
                    check_children(&mut tp, "expand-first");
                }

                let plain_shallow = api_stability_shared_type_entry(
                    &tp,
                    Some(plain_decl),
                    ApiStabilityInspection::Shallow,
                );
                if plain_shallow.is_none() {
                    t.fatal("the plain shallow surface must be published separately");
                }
                if api_stability_shared_entry_has_symbol(plain_shallow.as_ref(), Some(e_symbol)) {
                    t.fatal("the shallow snapshot must not contain expanded internals");
                }
                let tagged_shallow = api_stability_shared_type_entry(
                    &tp,
                    Some(tagged_decl),
                    ApiStabilityInspection::Shallow,
                );
                if tagged_shallow.is_none() {
                    t.fatal("the tagged shallow surface must be published separately");
                }
                if api_stability_shared_entry_has_symbol(tagged_shallow.as_ref(), Some(e_symbol)) {
                    t.fatal("the tagged shallow snapshot must not contain expanded internals");
                }
                if !api_stability_shared_entry_removed_symbol(tagged_shallow.as_ref(), Some(tagged))
                {
                    t.fatal("the tagged shallow snapshot must record the removed own tag");
                }
                let plain_expand_after = api_stability_shared_type_entry(
                    &tp,
                    Some(plain_decl),
                    ApiStabilityInspection::Expand,
                );
                if !api_stability_shared_entry_has_symbol(
                    plain_expand_after.as_ref(),
                    Some(e_symbol),
                ) {
                    t.fatal("the shallow composition contaminated the expanded snapshot");
                }
            });
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1229 TestApiStabilityConcreteInstantiationsAndCarriers 2a4123b202163cb0
#[test]
fn test_api_stability_concrete_instantiations_and_carriers() {
    run(|t| {
        t.parallel();
        for order in ["leaky-first", "clean-first"] {
            t.run(order, |t| {
                t.parallel();
                let leaky = "export declare const leaky: Box<E>";
                let clean = "export declare const clean: Box<string>";
                let mut declarations = format!("{leaky}\n{clean}");
                if order == "clean-first" {
                    declarations = format!("{clean}\n{leaky}");
                }
                let source = format!(
                    "/** @stability experimental */\nexport interface E {{ value: string }}\nexport interface Box<T> {{ value: T }}\n{declarations}\n"
                );
                let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                    t,
                    &[("/.src/test.ts", source.as_str())],
                );
                prime_api_stability_checker(t, &mut compiled);
                let test_file = files["/.src/test.ts"];
                let mut tp = compiled.type_parser();
                let leaky_symbol = export_of(t, &mut tp, test_file, "leaky");
                let clean_symbol = export_of(t, &mut tp, test_file, "clean");
                let leaky_type = tp.checker.get_type_of_symbol_exported(leaky_symbol);
                let clean_type = tp.checker.get_type_of_symbol_exported(clean_symbol);
                let e_symbol = export_of(t, &mut tp, test_file, "E");

                let leaky_used = tp.new_api_stability_session().used_by_symbol(&mut tp, leaky_symbol);
                if leaky_used.incomplete || !contains_dependency(&leaky_used, "E") {
                    t.fatal(format!("Box<E> should expose E, got {}", names(&leaky_used)));
                }
                let clean_used = tp.new_api_stability_session().used_by_symbol(&mut tp, clean_symbol);
                if contains_dependency(&clean_used, "E") {
                    t.fatal(format!("Box<string> must not inherit E, got {}", names(&clean_used)));
                }

                let leaky_entry = api_stability_shared_type_entry(
                    &tp,
                    Some(leaky_type),
                    ApiStabilityInspection::Expand,
                );
                let clean_entry = api_stability_shared_type_entry(
                    &tp,
                    Some(clean_type),
                    ApiStabilityInspection::Expand,
                );
                if leaky_entry.is_none() || clean_entry.is_none() {
                    t.fatal("both concrete instantiations must publish their own snapshot");
                }
                if !api_stability_shared_entry_has_symbol(leaky_entry.as_ref(), Some(e_symbol)) {
                    t.fatal("the Box<E> snapshot must keep E");
                }
                if api_stability_shared_entry_has_symbol(clean_entry.as_ref(), Some(e_symbol)) {
                    t.fatal("the Box<string> snapshot must not contain E");
                }

                let box_symbol = export_of(t, &mut tp, test_file, "Box");
                let box_decl = tp.checker.get_declared_type_of_symbol_exported(box_symbol);
                if api_stability_shared_type_entry(&tp, Some(box_decl), ApiStabilityInspection::Expand)
                    .is_some()
                {
                    t.fatal("a substituted raw declaration surface must stay analysis-local");
                }
                if api_stability_shared_type_entry(&tp, Some(box_decl), ApiStabilityInspection::Shallow)
                    .is_some()
                {
                    t.fatal("a substituted raw declaration surface must stay analysis-local");
                }
            });
        }

        t.run("substituted raw signature stays local", |t| {
            t.parallel();
            let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                t,
                &[(
                    "/.src/test.ts",
                    r"/** @stability experimental */
export interface E { value: string }
declare function make<T>(): (x: T) => T
export const leaky = make<E>()
",
                )],
            );
            prime_api_stability_checker(t, &mut compiled);
            let mut tp = compiled.type_parser();
            let leaky = export_of(t, &mut tp, files["/.src/test.ts"], "leaky");
            let instantiated = api_stability_test_signatures(t, tp.checker, Some(leaky))[0];
            let raw = raw_signature(Some(instantiated));
            if raw == Some(instantiated) {
                t.fatal("expected an instantiated signature with a raw target");
            }

            let mut analysis = new_api_stability_analysis();
            let subst = analysis
                .extend_signature_substitution(&ApiStabilitySubstitution::default(), instantiated);
            let inner = analysis.signature_surface(&mut tp, raw, &subst);
            analysis.settle(&mut tp);
            if api_stability_shared_signature_entry(&tp, raw).is_some() {
                t.fatal("a substituted raw signature surface must not be published");
            }
            let stored = api_stability_used_of(&inner, None);
            if stored.incomplete || !contains_dependency(&stored, "E") {
                t.fatal(format!(
                    "the substituted raw signature should expose E, got {}",
                    names(&stored)
                ));
            }

            let _ = tp.new_api_stability_session().used_by_signature(&mut tp, instantiated);
            if api_stability_shared_signature_entry(&tp, raw).is_some() {
                t.fatal("the concrete signature published under the raw signature slot");
            }
            if api_stability_shared_signature_entry(&tp, Some(instantiated)).is_none() {
                t.fatal("the concrete context-free signature should be published");
            }
        });
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1330 TestApiStabilitySnapshotMutationIsIsolated 2ed7fa4ebbbde98b
#[test]
fn test_api_stability_snapshot_mutation_is_isolated() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }
export interface Shared { leak: E }
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let e_symbol = export_of(t, &mut tp, test_file, "E");
        let shared = export_of(t, &mut tp, test_file, "Shared");
        let shared_decl = tp.checker.get_declared_type_of_symbol_exported(shared);

        let first = tp.new_api_stability_session().used_by_type(&mut tp, shared_decl);
        if first.incomplete || !contains_dependency(&first, "E") {
            t.fatal(format!("Shared root should expose E, got {}", names(&first)));
        }
        let entry =
            api_stability_shared_type_entry(&tp, Some(shared_decl), ApiStabilityInspection::Expand);
        if entry.is_none() || !api_stability_shared_entry_has_symbol(entry.as_ref(), Some(e_symbol))
        {
            t.fatal("the published snapshot should contain the E finding");
        }
        let published = entry.map_or(0, |entry| entry.findings.len());

        let mut session = tp.new_api_stability_session();
        let used = session.used_by_type(&mut tp, shared_decl);
        if used.incomplete || !contains_dependency(&used, "E") {
            t.fatal(format!("the reused root lost E, got {}", names(&used)));
        }
        let key = ApiStabilityTypeKey {
            t: shared_decl,
            inspect: ApiStabilityInspection::Expand,
            carrier: None,
        };
        // Rust: Go writes through the stored surface's shared map; the session surface's own copy is written here.
        let local =
            session.analysis.session_types.get_mut(&key).expect("the reused root is stored");
        let findings = std::rc::Rc::make_mut(&mut local.findings);
        findings.remove(&ApiStabilityFindingKey { symbol: Some(e_symbol), ..Default::default() });
        findings.insert(
            ApiStabilityFindingKey { declaration: Some(test_file.as_node()), ..Default::default() },
            ApiStabilityFinding { level: ApiStabilityLevel::Experimental, declaration: None },
        );

        let after =
            api_stability_shared_type_entry(&tp, Some(shared_decl), ApiStabilityInspection::Expand);
        if after.as_ref().is_none_or(|after| after.findings.len() != published) {
            t.fatal(format!(
                "the published snapshot changed after local mutation: before={published} after={}",
                after.as_ref().map_or(0, |after| after.findings.len())
            ));
        }
        if !api_stability_shared_entry_has_symbol(after.as_ref(), Some(e_symbol)) {
            t.fatal("local mutation removed a finding from the published snapshot");
        }

        let mut found_provenance = false;
        for dependency in &first.dependencies {
            if api_stability_dependency_name(dependency) == "E" && dependency.declaration.is_some()
            {
                found_provenance = true;
            }
        }
        if !found_provenance {
            t.fatal("a snapshot finding lost its declaration provenance");
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1385 TestApiStabilityIncompleteAndBlockedAreNeverPublished 417dd8269367988f
#[test]
fn test_api_stability_incomplete_and_blocked_are_never_published() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Cold { plain: number; [Symbol.iterator](): E }
",
            )],
        );
        let test_file = files["/.src/test.ts"];
        let (cold, cold_decl) = {
            let mut tp = compiled.type_parser();
            let cold = export_of(t, &mut tp, test_file, "Cold");
            let cold_decl = tp.checker.get_declared_type_of_symbol_exported(cold);

            let cold_used = tp.new_api_stability_session().used_by_symbol(&mut tp, cold);
            if !cold_used.incomplete {
                t.fatal("an unmaterialized computed member must report an incomplete result");
            }
            assert_api_stability_surface_excludes_own_tag(t, &cold_used, "Cold");
            if api_stability_shared_type_entry(&tp, Some(cold_decl), ApiStabilityInspection::Expand)
                .is_some()
            {
                t.fatal("an incomplete cold surface must not be published");
            }

            let mut exhausted = new_api_stability_analysis();
            exhausted.work = API_STABILITY_MAX_WORK;
            let blocked = exhausted.type_surface(
                &mut tp,
                Some(cold_decl),
                ApiStabilityInspection::Expand,
                &ApiStabilitySubstitution::default(),
            );
            exhausted.settle(&mut tp);
            if !blocked.blocked {
                t.fatal("an exhausted read must be blocked");
            }
            if api_stability_shared_type_entry(&tp, Some(cold_decl), ApiStabilityInspection::Expand)
                .is_some()
            {
                t.fatal("a budget-blocked surface must not be published");
            }
            (cold, cold_decl)
        };

        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();
        let warm = tp.new_api_stability_session().used_by_symbol(&mut tp, cold);
        if warm.incomplete || !contains_dependency(&warm, "E") {
            t.fatal(format!("the materialized retry should expose E, got {}", names(&warm)));
        }
        assert_api_stability_surface_excludes_own_tag(t, &warm, "Cold");
        if api_stability_shared_type_entry(&tp, Some(cold_decl), ApiStabilityInspection::Expand)
            .is_none()
        {
            t.fatal("the complete retry must publish its snapshot");
        }
        let mut reuse = new_api_stability_analysis();
        reuse.type_surface(
            &mut tp,
            Some(cold_decl),
            ApiStabilityInspection::Expand,
            &ApiStabilitySubstitution::default(),
        );
        if reuse.work != 0 {
            t.fatal(format!("the published retry snapshot consumed {} work", reuse.work));
        }
        let reused = tp.new_api_stability_session().used_by_symbol(&mut tp, cold);
        if reused.incomplete
            || !contains_dependency(&reused, "E")
            || reused.dependencies.len() != warm.dependencies.len()
        {
            t.fatal(format!("reused surface disagrees: {}/{}", lv(reused.minimum), names(&reused)));
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1441 TestApiStabilityCyclesPublishOnlySettledComplete 2ff0d5d70883a76c
#[test]
fn test_api_stability_cycles_publish_only_settled_complete() {
    run(|t| {
        t.parallel();
        let fixture: &[(&str, &str)] = &[(
            "/.src/test.ts",
            r"/** @stability experimental */
interface E { value: string }
export interface ColdA { b: ColdB }
export interface ColdB { a: ColdA; payload: E; [Symbol.iterator](): E }
",
        )];
        for order in ["b-first", "a-first"] {
            t.run(order, |t| {
                t.parallel();
                let (mut compiled, files) =
                    compile_and_get_checker_and_source_files_internal(t, fixture);
                let test_file = files["/.src/test.ts"];
                let (a, b, b_decl) = {
                    let mut tp = compiled.type_parser();
                    let a = export_of(t, &mut tp, test_file, "ColdA");
                    let b = export_of(t, &mut tp, test_file, "ColdB");
                    let b_decl = tp.checker.get_declared_type_of_symbol_exported(b);

                    let (cold_a, cold_b) = if order == "b-first" {
                        let cold_b = tp.new_api_stability_session().used_by_symbol(&mut tp, b);
                        let cold_a = tp.new_api_stability_session().used_by_symbol(&mut tp, a);
                        (cold_a, cold_b)
                    } else {
                        let cold_a = tp.new_api_stability_session().used_by_symbol(&mut tp, a);
                        let cold_b = tp.new_api_stability_session().used_by_symbol(&mut tp, b);
                        (cold_a, cold_b)
                    };
                    if !cold_b.incomplete {
                        t.fatal("the cold computed member must keep ColdB incomplete");
                    }
                    if api_stability_shared_type_entry(
                        &tp,
                        Some(b_decl),
                        ApiStabilityInspection::Expand,
                    )
                    .is_some()
                    {
                        t.fatal("an incomplete cold cycle surface must not be published");
                    }
                    if cold_a.incomplete || contains_dependency(&cold_a, "E") {
                        t.fatal(format!(
                            "ColdA = {}/{}, want a complete shallow surface",
                            lv(cold_a.minimum),
                            names(&cold_a)
                        ));
                    }
                    (a, b, b_decl)
                };

                prime_api_stability_checker(t, &mut compiled);
                let mut tp = compiled.type_parser();
                let warm_b = tp.new_api_stability_session().used_by_symbol(&mut tp, b);
                if warm_b.incomplete
                    || warm_b.minimum != ApiStabilityLevel::Experimental
                    || !contains_dependency(&warm_b, "E")
                {
                    t.fatal(format!(
                        "warm ColdB = {}/{}, want experimental E",
                        lv(warm_b.minimum),
                        names(&warm_b)
                    ));
                }
                if api_stability_shared_type_entry(
                    &tp,
                    Some(b_decl),
                    ApiStabilityInspection::Expand,
                )
                .is_none()
                {
                    t.fatal("the warm complete cycle must publish its snapshot");
                }
                let warm_a = tp.new_api_stability_session().used_by_symbol(&mut tp, a);
                if warm_a.incomplete || contains_dependency(&warm_a, "E") {
                    t.fatal(format!(
                        "warm ColdA = {}/{}, want only the shallow surface",
                        lv(warm_a.minimum),
                        names(&warm_a)
                    ));
                }
                let mut reuse = new_api_stability_analysis();
                reuse.type_surface(
                    &mut tp,
                    Some(b_decl),
                    ApiStabilityInspection::Expand,
                    &ApiStabilitySubstitution::default(),
                );
                if reuse.work != 0 {
                    t.fatal(format!("the published cycle snapshot consumed {} work", reuse.work));
                }
            });
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1501 TestApiStabilityOverloadTagsSurviveSnapshotReuse b1d96a53c36e89aa
#[test]
fn test_api_stability_overload_tags_survive_snapshot_reuse() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r#"/** @stability unstable */
export function mixed(x: string): string
/** @stability experimental */
export function mixed(x: number): number
export function mixed(x: string | number): string | number { return "" }
"#,
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();
        let mixed = export_of(t, &mut tp, files["/.src/test.ts"], "mixed");
        let signatures = api_stability_test_signatures(t, tp.checker, Some(mixed));
        if signatures.len() != 2 {
            t.fatal(format!("expected two public overloads, got {}", signatures.len()));
        }

        let first = tp.new_api_stability_session().used_by_symbol(&mut tp, mixed);
        if first.incomplete || first.minimum != ApiStabilityLevel::Experimental {
            t.fatal(format!(
                "mixed = {}/{}, want experimental from the sibling overload",
                lv(first.minimum),
                names(&first)
            ));
        }
        for dependency in &first.dependencies {
            if dependency.signature.is_some()
                && raw_signature(dependency.signature) == raw_signature(Some(signatures[0]))
            {
                t.fatal(format!(
                    "the declared own overload leaked into the surface: {}",
                    names(&first)
                ));
            }
        }
        if api_stability_shared_signature_entry(&tp, Some(signatures[0])).is_none()
            || api_stability_shared_signature_entry(&tp, Some(signatures[1])).is_none()
        {
            t.fatal("both overload signature surfaces should be published");
        }
        for (index, &signature) in signatures.iter().enumerate() {
            let entry = api_stability_shared_signature_entry(&tp, Some(signature));
            if !api_stability_shared_entry_removed_signature(entry.as_ref(), Some(signature)) {
                t.fatal(format!("overload {index} own tag must be removed from its snapshot"));
            }
        }

        let reused = tp.new_api_stability_session().used_by_symbol(&mut tp, mixed);
        if reused.incomplete
            || reused.minimum != first.minimum
            || reused.dependencies.len() != first.dependencies.len()
        {
            t.fatal(format!(
                "reused mixed = {}/{}, want {}",
                lv(reused.minimum),
                names(&reused),
                names(&first)
            ));
        }
        for (index, &signature) in signatures.iter().enumerate() {
            let used = tp.new_api_stability_session().used_by_signature(&mut tp, signature);
            if used.incomplete
                || used.minimum != ApiStabilityLevel::Stable
                || !used.dependencies.is_empty()
            {
                t.fatal(format!(
                    "reused overload {index} own tag must be excluded, got min={} deps={}",
                    lv(used.minimum),
                    names(&used)
                ));
            }
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1551 TestApiStabilityColdCompleteAgreesWithPureWarm 4de0664e4b5f2756
#[test]
fn test_api_stability_cold_complete_agrees_with_pure_warm() {
    run(|t| {
        t.parallel();
        let source = r"/** @stability experimental */
interface E { value: string }
type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<E>>
";
        let (mut cold, cold_files) =
            compile_and_get_checker_and_source_files_internal(t, &[("/.src/test.ts", source)]);
        let cold_symbol = {
            let mut tp_cold = cold.type_parser();
            let cold_symbol = export_of(t, &mut tp_cold, cold_files["/.src/test.ts"], "f");
            let cold_used =
                tp_cold.new_api_stability_session().used_by_symbol(&mut tp_cold, cold_symbol);
            if cold_used.incomplete || !contains_dependency(&cold_used, "E") {
                t.fatal(format!(
                    "the cold surface should lazily materialize E, got {}",
                    names(&cold_used)
                ));
            }
            let cold_type = tp_cold.checker.get_type_of_symbol_exported(cold_symbol);
            if api_stability_shared_type_entry(
                &tp_cold,
                Some(cold_type),
                ApiStabilityInspection::Expand,
            )
            .is_none()
            {
                t.fatal("the cold complete surface should be published");
            }
            cold_symbol
        };

        prime_api_stability_checker(t, &mut cold);
        let mut tp_reused = new_type_parser(cold.program, &mut cold.checker);
        let reused =
            tp_reused.new_api_stability_session().used_by_symbol(&mut tp_reused, cold_symbol);
        if reused.incomplete {
            t.fatal("the reused snapshot must stay complete");
        }

        let (mut warm, warm_files) =
            compile_and_get_checker_and_source_files_internal(t, &[("/.src/test.ts", source)]);
        prime_api_stability_checker(t, &mut warm);
        let mut tp_warm = warm.type_parser();
        let warm_symbol = export_of(t, &mut tp_warm, warm_files["/.src/test.ts"], "f");
        let pure = tp_warm.new_api_stability_session().used_by_symbol(&mut tp_warm, warm_symbol);
        if pure.incomplete {
            t.fatal("the pure warm recomputation must be complete");
        }

        if reused.minimum != pure.minimum {
            t.fatal(format!(
                "reused minimum {} disagrees with pure warm {}",
                lv(reused.minimum),
                lv(pure.minimum)
            ));
        }
        let reused_names = api_stability_dependency_names(&reused);
        let pure_names = api_stability_dependency_names(&pure);
        if reused_names.len() != pure_names.len() {
            t.fatal(format!(
                "reused dependencies {reused_names:?} disagree with pure warm {pure_names:?}"
            ));
        }
        for index in 0..reused_names.len() {
            if reused_names[index] != pure_names[index] {
                t.fatal(format!(
                    "reused dependencies {reused_names:?} disagree with pure warm {pure_names:?}"
                ));
            }
        }
    });
}

// go test: internal/typeparser/api_stability_surface_test.go:1601 TestApiStabilitySymbolLessTypesReuse bcb56a0c1ba040fb
#[test]
fn test_api_stability_symbol_less_types_reuse() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r"/** @stability experimental */
export interface E { value: string }
export interface Plain { value: string }
export declare const choose: E | Plain
",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        let choose = export_of(t, &mut tp, test_file, "choose");
        let union = tp.checker.get_type_of_symbol_exported(choose);
        if !union.flags().intersects(ets_shim::checker::TypeFlags::UnionOrIntersection) {
            t.fatal(format!("expected a union type, got flags {:?}", union.flags()));
        }

        let first = tp.new_api_stability_session().used_by_type(&mut tp, union);
        if first.incomplete
            || first.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&first, "E")
        {
            t.fatal(format!(
                "union root = {}/{}, want experimental E",
                lv(first.minimum),
                names(&first)
            ));
        }
        let entry =
            api_stability_shared_type_entry(&tp, Some(union), ApiStabilityInspection::Expand);
        let Some(entry) = entry else {
            t.fatal("a symbol-less complete surface must be published");
        };
        if !entry.removed.is_empty() {
            t.fatal(format!(
                "a symbol-less type has no own tag to remove, got {:?}",
                entry.removed
            ));
        }
        let e_symbol = export_of(t, &mut tp, test_file, "E");
        if !api_stability_shared_entry_has_symbol(Some(&entry), Some(e_symbol)) {
            t.fatal("the union snapshot must keep its component finding");
        }

        let reused = tp.new_api_stability_session().used_by_type(&mut tp, union);
        if reused.incomplete
            || reused.minimum != first.minimum
            || reused.dependencies.len() != first.dependencies.len()
        {
            t.fatal(format!(
                "reused union = {}/{}, want {}",
                lv(reused.minimum),
                names(&reused),
                names(&first)
            ));
        }
        let mut direct = new_api_stability_analysis();
        direct.type_surface(
            &mut tp,
            Some(union),
            ApiStabilityInspection::Expand,
            &ApiStabilitySubstitution::default(),
        );
        if direct.work != 0 {
            t.fatal(format!("the symbol-less snapshot hit consumed {} work", direct.work));
        }
    });
}
