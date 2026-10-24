use std::sync::Arc;

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::{CompilerOptions, P, ScriptTarget, Tristate};
use ets_shim::vfs::vfstest::MapFile;

use crate::helpers_test::*;
use crate::native_string_test::same_arc;
use crate::*;

// go test: internal/typeparser/native_array_test.go:17 findNativeArrayAccess
fn find_native_array_access(t: &T, sf: P<SourceFile>, text: &str) -> P<Node> {
    t.helper();
    fn walk(sf: P<SourceFile>, node: P<Node>, text: &str, found: &mut Option<P<Node>>) -> bool {
        if node.kind() == Kind::PropertyAccessExpression
            || node.kind() == Kind::ElementAccessExpression
        {
            let pos = node.pos().source_offset().expect("a parsed node has a position") as usize;
            let end = node.end().source_offset().expect("a parsed node has a position") as usize;
            if sf.text()[pos..end].trim() == text {
                *found = Some(node);
                return false;
            }
        }
        node.for_each_child(&mut |child| walk(sf, child, text, found));
        false
    }
    let mut found = None;
    walk(sf, sf.as_node(), text, &mut found);
    let Some(found) = found else {
        t.fatal(sprintf!("member access %q not found", text));
    };
    found
}

/// Rust-only: Go's method values `tp.IsNativeArrayMethodReference` / `tp.IsNativeArrayConstructorMethodReference`.
type NativeArrayQuery = fn(&mut TypeParser<'_>, P<Node>, &str) -> bool;

// go test: internal/typeparser/native_array_test.go:37 TestIsArrayType
#[test]
fn test_is_array_type() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"
type Numbers = number[];
declare const mutable: number[];
declare const readonly: readonly number[];
declare const alias: Numbers;
declare const tuple: [number, string];
declare const readonlyTuple: readonly [number, string];
declare const union: number[] | string[];
declare const optional: number[] | undefined;
declare const typed: Uint8Array;
declare const anyValue: any;
class Inherited extends Array<number> {}
declare const inherited: Inherited;
function constrained<T extends readonly number[]>(generic: T) {}
",
        );
        let mut tp = compiled.type_parser();
        for (name, want) in [
            ("mutable", true),
            ("readonly", true),
            ("alias", true),
            ("tuple", false),
            ("readonlyTuple", false),
            ("union", false),
            ("optional", false),
            ("typed", false),
            ("anyValue", false),
            ("inherited", false),
            ("generic", false),
        ] {
            let typ = tp.get_type_at_location(find_identifier_by_text(t, sf, name, 0));
            let got = tp.is_array_type(typ);
            if got != want {
                t.error(sprintf!("IsArrayType(%s) = %v, want %v", name, got, want));
            }
        }
        if tp.is_array_type(None) {
            t.error("IsArrayType(nil) = true");
        }
    });
}

// go test: internal/typeparser/native_array_test.go:74 TestNativeArrayMethodReference
#[test]
fn test_native_array_method_reference() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r#"
declare const values: number[];
declare const readonly: readonly number[];
declare const tuple: readonly [number, string];
declare const optional: readonly number[] | undefined;
declare const nativeUnion: number[] | readonly string[];
declare const mixed: number[] | { map: (f: (n: number) => number) => number[] };
class Inherited extends Array<number> {}
declare const inherited: Inherited;
class Override extends Array<number> { map<U>(f: (n: number, i: number, a: number[]) => U): U[] { return []; } }
declare const override: Override;
declare const typed: Uint8Array;
declare const fn: () => void;
declare const object: object;
declare const key: "map";
type Remapped = { [K in keyof number[] as K extends "filter" ? "map" : never]: number[][K] };
declare const remapped: Remapped;
declare const conflicting: Remapped | number[];
function constrained<T extends readonly number[]>(generic: T) { generic["map"]; }
function shadowed(Array: { isArray: (value: unknown) => boolean }) { Array.isArray; }
const Native = Array;
values.map;
values.push;
readonly.filter;
tuple.map;
tuple["map"];
optional?.filter;
optional?.["filter"];
values[`map`];
nativeUnion.map;
mixed.map;
inherited.map;
override.map;
typed.map;
fn.call;
object.hasOwnProperty;
Native.isArray;
Native["isArray"];
Native.from;
Native.call;
values[key];
remapped.map;
conflicting.map;
const alias = values.map;
alias;
"#,
        );
        let mut tp = compiled.type_parser();
        for (access, name, constructor, want) in [
            ("Array.isArray", "isArray", true, false),
            ("values.map", "map", false, true),
            ("values.push", "push", false, true),
            ("readonly.filter", "filter", false, true),
            ("tuple.map", "map", false, true),
            ("tuple[\"map\"]", "map", false, true),
            ("optional?.filter", "filter", false, true),
            ("optional?.[\"filter\"]", "filter", false, true),
            ("values[`map`]", "map", false, true),
            ("generic[\"map\"]", "map", false, true),
            ("nativeUnion.map", "map", false, true),
            ("mixed.map", "map", false, false),
            ("inherited.map", "map", false, true),
            ("override.map", "map", false, false),
            ("typed.map", "map", false, false),
            ("fn.call", "call", true, false),
            ("fn.call", "call", false, false),
            ("object.hasOwnProperty", "hasOwnProperty", false, false),
            ("Native.isArray", "isArray", true, true),
            ("Native[\"isArray\"]", "isArray", true, true),
            ("Native.from", "from", true, true),
            ("Native.call", "call", true, false),
            ("values[key]", "map", false, false),
            ("remapped.map", "filter", false, true),
            ("remapped.map", "map", false, false),
            ("conflicting.map", "map", false, false),
            ("conflicting.map", "filter", false, false),
        ] {
            let node = find_native_array_access(t, sf, access);
            let (mut query, mut other_namespace): (NativeArrayQuery, NativeArrayQuery) = (
                |tp, node, name| tp.is_native_array_method_reference(node, name),
                |tp, node, name| tp.is_native_array_constructor_method_reference(node, name),
            );
            if constructor {
                (query, other_namespace) = (other_namespace, query);
            }
            if query(&mut tp, node, "unknownMethod") {
                t.error(sprintf!("%s matched an unknown method before classification", access));
            }
            let got = query(&mut tp, node, name);
            if got != want {
                t.error(sprintf!("%s as %s = %v, want %v", access, name, got, want));
            }
            if query(&mut tp, node, "unknownMethod") || other_namespace(&mut tp, node, name) {
                t.error(sprintf!(
                    "%s classification leaked into another name or namespace",
                    access
                ));
            }
            let got = query(&mut tp, node, name);
            if got != want {
                t.error(sprintf!("%s cached as %s = %v, want %v", access, name, got, want));
            }
        }
        if tp.is_native_array_method_reference(None, "map")
            || tp
                .is_native_array_method_reference(find_identifier_by_text(t, sf, "alias", 1), "map")
            || tp
                .is_native_array_method_reference(find_native_array_access(t, sf, "values.map"), "")
        {
            t.error("unsupported reference matched a native Array method");
        }
    });
}

// go test: internal/typeparser/native_array_test.go:182 TestNativeArrayAugmentation
#[test]
fn test_native_array_augmentation() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"
export {};
declare global {
  interface Array<T> { map<U>(f: (value: T) => U): U[]; }
  interface ReadonlyArray<T> { map<U>(f: (value: T) => U): U[]; }
  interface ArrayConstructor { isArray(value: number): boolean; }
}
declare const values: number[];
declare const readonly: readonly number[];
values.map;
readonly.map;
values.filter;
readonly.filter;
Array.isArray;
Array.of;
",
        );
        let mut tp = compiled.type_parser();
        for access in ["values.map", "readonly.map"] {
            if tp.is_native_array_method_reference(find_native_array_access(t, sf, access), "map") {
                t.error(sprintf!("augmented member %s matched", access));
            }
        }
        for access in ["values.filter", "readonly.filter"] {
            if !tp
                .is_native_array_method_reference(find_native_array_access(t, sf, access), "filter")
            {
                t.error(sprintf!("unaugmented member %s did not match", access));
            }
        }
        if tp.is_native_array_constructor_method_reference(
            find_native_array_access(t, sf, "Array.isArray"),
            "isArray",
        ) || !tp.is_native_array_constructor_method_reference(
            find_native_array_access(t, sf, "Array.of"),
            "of",
        ) {
            t.error("constructor augmentation affected the wrong members");
        }
    });
}

// go test: internal/typeparser/native_array_test.go:217 TestNativeArrayInheritedAugmentation
#[test]
fn test_native_array_inherited_augmentation() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r#"
export {};
declare global {
  interface Array<T> extends Pick<ArrayConstructor, "isArray"> {}
}
Array.isArray;
"#,
        );
        let mut tp = compiled.type_parser();
        let node = find_native_array_access(t, sf, "Array.isArray");
        if !tp.is_native_array_constructor_method_reference(node, "isArray") {
            t.error("Array.isArray constructor reference = false, want true");
        }
        if tp.is_native_array_method_reference(node, "isArray") {
            t.error("Array.isArray instance reference = true, want false");
        }
    });
}

// go test: internal/typeparser/native_array_test.go:236 TestNativeArrayNoLib
// Rust: Go's last check (`len(tp.links.nativeArrayRegistry.instance) != 0 || len(...constructor) != 0`, "noLib
// registry contains canonical members") is left out: the registry's fields are private to `native_array`.
#[test]
fn test_native_array_no_lib() {
    run(|t| {
        t.parallel();
        let source = r"
interface Array<T> { map(f: (value: T) => T): T[]; }
interface ArrayConstructor { isArray(value: unknown): boolean; }
declare const Array: ArrayConstructor;
declare const values: number[];
values.map;
Array.isArray;
";
        let mut testfs = TestFS::default();
        testfs.insert("/.src/test.ts".to_string(), MapFile::from(source));
        let mut compiled = new_compiled(
            testfs,
            CompilerOptions {
                no_lib: Tristate::True,
                target: ScriptTarget::ESNext,
                strict: Tristate::True,
                ..Default::default()
            },
            vec!["/.src/test.ts".to_string()],
        );
        let sf = compiled.program.get_source_file("/.src/test.ts").expect("the root file");
        let mut tp = compiled.type_parser();
        if tp.is_native_array_method_reference(find_native_array_access(t, sf, "values.map"), "map")
            || tp.is_native_array_constructor_method_reference(
                find_native_array_access(t, sf, "Array.isArray"),
                "isArray",
            )
        {
            t.error("noLib user declarations matched native Array members");
        }
    });
}

// go test: internal/typeparser/native_array_test.go:269 TestNativeArrayCheckerCache
// Rust: a parser borrows its checker mutably, so `tp` and `second` (both `new_type_parser` over the same checker,
// as in Go) are re-created for each use; they share the checker's links. Go compares the cached member pointers;
// they come from one links store, so the values are compared. The member's fields are private to `native_array`,
// so the expected member is compared through its derived `Debug` form.
#[test]
fn test_native_array_checker_cache() {
    run(|t| {
        t.parallel();
        let source = "export {};
declare const values: number[];
declare const custom: { map: () => void };
values.map;
custom.map;
";
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[("/.src/first.ts", source), ("/.src/second.ts", source)],
        );
        let first = files["/.src/first.ts"];
        let tp_links = Arc::clone(&compiled.type_parser().links);
        if tp_links.lock().native_array_registry.is_some() {
            t.fatal("registry was built before a reference query");
        }
        {
            let mut tp = compiled.type_parser();
            let typ = tp.get_type_at_location(find_identifier_by_text(t, first, "values", 0));
            if !tp.is_array_type(typ) || tp.links().native_array_registry.is_some() {
                t.fatal("IsArrayType changed reference registry initialization");
            }
        }
        for access in ["values.map", "custom.map"] {
            let node = find_native_array_access(t, first, access);
            let want = access == "values.map";
            let mut tp = compiled.type_parser();
            let got = tp.is_native_array_method_reference(node, "map");
            if got != want {
                t.fatal(sprintf!("first parser %s = %v, want %v", access, got, want));
            }
            let symbol = tp.get_symbol_at_location(node);
            let cached = symbol.and_then(|s| tp.links().native_array_member.get(&s).cloned());
            let want_member = if want {
                format!(
                    "NativeArrayMember {{ namespace: {:?}, name: \"map\" }}",
                    NativeArrayNamespace::Instance
                )
            } else {
                format!("{:?}", NativeArrayMember::default())
            };
            if cached.as_ref().map(|m| format!("{m:?}")) != Some(want_member) {
                t.fatal(sprintf!("%s classification was not cached", access));
            }
            let mut second = compiled.type_parser();
            let got = second.is_native_array_method_reference(node, "map");
            if got != want
                || !Arc::ptr_eq(&second.links, &tp_links)
                || symbol.and_then(|s| second.links().native_array_member.get(&s).cloned())
                    != cached
            {
                t.fatal(sprintf!("second parser did not reuse the %s classification", access));
            }
        }
        let registry = tp_links.lock().native_array_registry.clone();
        let mut second = compiled.type_parser();
        if !second.is_native_array_method_reference(
            find_native_array_access(t, files["/.src/second.ts"], "values.map"),
            "map",
        ) || !same_arc(second.links().native_array_registry.as_ref(), registry.as_ref())
        {
            t.fatal("second source file did not reuse the canonical registry");
        }
        let (mut other_compiled, other_file) =
            compile_and_get_checker_and_source_file_internal(t, source);
        let mut other = other_compiled.type_parser();
        let first_symbol = compiled.type_parser().get_symbol_at_location(find_native_array_access(
            t,
            first,
            "values.map",
        ));
        if Arc::ptr_eq(&other.links, &tp_links)
            || other.links().native_array_registry.is_some()
            || first_symbol.is_some_and(|s| other.links().native_array_member.contains_key(&s))
        {
            t.fatal("separate checker inherited native Array cache entries");
        }
        let other_node = find_native_array_access(t, other_file, "values.map");
        if !other.is_native_array_method_reference(other_node, "map")
            || same_arc(other.links().native_array_registry.as_ref(), registry.as_ref())
            || other.get_symbol_at_location(other_node) == first_symbol
        {
            t.fatal("separate checker did not build its own native Array cache");
        }
    });
}
