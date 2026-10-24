use std::sync::Arc;

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::{CompilerOptions, P, ScriptTarget, Tristate};
use ets_shim::vfs::vfstest::MapFile;

use crate::helpers_test::*;
use crate::native_string_test::same_arc;
use crate::*;

// go test: internal/typeparser/native_object_test.go:17 findNativeObjectAccess
fn find_native_object_access(t: &T, sf: P<SourceFile>, text: &str) -> P<Node> {
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

/// Rust-only: Go's method values `tp.IsNativeObjectMethodReference` / `tp.IsNativeObjectConstructorMethodReference`.
type NativeObjectQuery = fn(&mut TypeParser<'_>, P<Node>, &str) -> bool;

// go test: internal/typeparser/native_object_test.go:37 TestNativeObjectMethodReference
#[test]
fn test_native_object_method_reference() {
    run(|t| {
        t.parallel();
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                ("/.src/aliases.ts", "export const Native = Object;"),
                (
                    "/.src/test.ts",
                    r#"
import { Native as Imported } from "./aliases";
declare const object: object;
declare const optional: object | undefined;
declare const optionalConstructor: ObjectConstructor | undefined;
declare const picked: Pick<ObjectConstructor, "keys">;
declare const mixed: ObjectConstructor | { keys: typeof Object.keys };
declare const mixedInstance: object | { hasOwnProperty: (key: PropertyKey) => boolean };
declare const intersection: ObjectConstructor & { keys: (value: number) => string[] };
declare const custom: { keys: typeof Object.keys };
declare const dynamic: "keys";
declare const anything: any;
declare const arrays: number[];
declare const map: Map<string, number>;
class Inherited {}
class Override { hasOwnProperty(key: PropertyKey): boolean { return false; } }
declare const inherited: Inherited;
declare const override: Override;
type Remapped = { [K in keyof ObjectConstructor as K extends "values" ? "keys" : never]: ObjectConstructor[K] };
declare const remapped: Remapped;
declare const conflicting: Remapped | Pick<ObjectConstructor, "keys">;
function constrained<T extends ObjectConstructor>(generic: T) { generic["entries"]; }
function constrainedInstance<T extends object>(genericInstance: T) { genericInstance["hasOwnProperty"]; }
function customConstraint<T extends { keys: (value: object) => string[] }>(customGeneric: T) { customGeneric["keys"]; }
function mixedConstraint<T extends ObjectConstructor | { keys: (value: object) => string[] }>(mixedGeneric: T) { mixedGeneric["keys"]; }
function shadowed(Object: { keys: typeof globalThis.Object.keys }) { Object?.keys; }
namespace Local {
  interface ObjectConstructor { keys(value: object): string[]; }
  declare const local: ObjectConstructor;
  local.keys;
}
const Native = Object;
Object.keys;
Object.values;
Object.entries;
Object.fromEntries;
Object.hasOwn;
Object["keys"];
Object[`values`];
Object.prototype.hasOwnProperty.call({}, "a");
Object.hasOwnProperty;
object.hasOwnProperty;
object["hasOwnProperty"];
optional?.hasOwnProperty;
optional?.["hasOwnProperty"];
optionalConstructor?.keys;
optionalConstructor?.["keys"];
inherited.hasOwnProperty;
override.hasOwnProperty;
Native.keys;
Native["entries"];
Imported.keys;
globalThis.Object.hasOwn;
globalThis.Object.keys;
picked.keys;
custom.keys;
mixed.keys;
mixedInstance.hasOwnProperty;
intersection.keys;
remapped.keys;
conflicting.keys;
Object[dynamic];
anything.keys;
arrays.keys;
arrays.entries;
map.keys;
const alias = Object.keys;
alias;
"#,
                ),
            ],
        );
        let sf = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();
        for (access, name, constructor, want) in [
            ("Object.keys", "keys", true, true),
            ("Object?.keys", "keys", true, false),
            ("globalThis.Object.keys", "keys", true, true),
            ("Object.values", "values", true, true),
            ("Object.entries", "entries", true, true),
            ("Object.fromEntries", "fromEntries", true, true),
            ("Object.hasOwn", "hasOwn", true, true),
            ("Object[\"keys\"]", "keys", true, true),
            ("Object[`values`]", "values", true, true),
            ("Object.prototype.hasOwnProperty", "hasOwnProperty", false, true),
            ("Object.hasOwnProperty", "hasOwnProperty", false, true),
            ("object.hasOwnProperty", "hasOwnProperty", false, true),
            ("object[\"hasOwnProperty\"]", "hasOwnProperty", false, true),
            ("optional?.hasOwnProperty", "hasOwnProperty", false, true),
            ("optional?.[\"hasOwnProperty\"]", "hasOwnProperty", false, true),
            ("optionalConstructor?.keys", "keys", true, true),
            ("optionalConstructor?.[\"keys\"]", "keys", true, true),
            ("inherited.hasOwnProperty", "hasOwnProperty", false, true),
            ("override.hasOwnProperty", "hasOwnProperty", false, false),
            ("Native.keys", "keys", true, true),
            ("Native[\"entries\"]", "entries", true, true),
            ("Imported.keys", "keys", true, true),
            ("globalThis.Object.hasOwn", "hasOwn", true, true),
            ("picked.keys", "keys", true, true),
            ("custom.keys", "keys", true, false),
            ("mixed.keys", "keys", true, false),
            ("mixedInstance.hasOwnProperty", "hasOwnProperty", false, false),
            ("intersection.keys", "keys", true, false),
            ("remapped.keys", "values", true, true),
            ("remapped.keys", "keys", true, false),
            ("conflicting.keys", "keys", true, false),
            ("conflicting.keys", "values", true, false),
            ("generic[\"entries\"]", "entries", true, true),
            ("genericInstance[\"hasOwnProperty\"]", "hasOwnProperty", false, true),
            ("customGeneric[\"keys\"]", "keys", true, false),
            ("mixedGeneric[\"keys\"]", "keys", true, false),
            ("local.keys", "keys", true, false),
            ("Object[dynamic]", "keys", true, false),
            ("anything.keys", "keys", true, false),
            ("arrays.keys", "keys", true, false),
            ("arrays.entries", "entries", true, false),
            ("map.keys", "keys", true, false),
        ] {
            let node = find_native_object_access(t, sf, access);
            let (mut query, mut other_namespace): (NativeObjectQuery, NativeObjectQuery) = (
                |tp, node, name| tp.is_native_object_method_reference(node, name),
                |tp, node, name| tp.is_native_object_constructor_method_reference(node, name),
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
        if tp.is_native_object_method_reference(None, "hasOwnProperty")
            || tp.is_native_object_constructor_method_reference(
                find_identifier_by_text(t, sf, "alias", 1),
                "keys",
            )
            || tp.is_native_object_constructor_method_reference(
                find_native_object_access(t, sf, "Native.keys"),
                "",
            )
        {
            t.error("unsupported reference matched a native Object method");
        }
        if !tp.is_native_array_method_reference(
            find_native_object_access(t, sf, "arrays.keys"),
            "keys",
        ) || tp.is_native_array_method_reference(
            find_native_object_access(t, sf, "Native.keys"),
            "keys",
        ) || tp.is_native_array_constructor_method_reference(
            find_native_object_access(t, sf, "Native.keys"),
            "keys",
        ) {
            t.error("Object recognition changed Array classification");
        }
    });
}

// go test: internal/typeparser/native_object_test.go:192 TestNativeObjectAugmentation
#[test]
fn test_native_object_augmentation() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"
export {};
declare global {
  interface ObjectConstructor { keys<T>(value: T): (keyof T)[]; }
  interface Object { hasOwnProperty(key: string): boolean; }
}
Object.keys;
Object.values;
Object.hasOwn;
Object.prototype.hasOwnProperty;
Object.prototype.isPrototypeOf;
",
        );
        let mut tp = compiled.type_parser();
        if tp.is_native_object_constructor_method_reference(
            find_native_object_access(t, sf, "Object.keys"),
            "keys",
        ) || !tp.is_native_object_constructor_method_reference(
            find_native_object_access(t, sf, "Object.values"),
            "values",
        ) || !tp.is_native_object_constructor_method_reference(
            find_native_object_access(t, sf, "Object.hasOwn"),
            "hasOwn",
        ) {
            t.error("constructor augmentation affected the wrong members");
        }
        if tp.is_native_object_method_reference(
            find_native_object_access(t, sf, "Object.prototype.hasOwnProperty"),
            "hasOwnProperty",
        ) || !tp.is_native_object_method_reference(
            find_native_object_access(t, sf, "Object.prototype.isPrototypeOf"),
            "isPrototypeOf",
        ) {
            t.error("instance augmentation affected the wrong members");
        }
    });
}

// go test: internal/typeparser/native_object_test.go:218 TestNativeObjectInheritedAugmentation
#[test]
fn test_native_object_inherited_augmentation() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r#"
export {};
declare global { interface Object extends Pick<ObjectConstructor, "keys"> {} }
Object.keys;
"#,
        );
        let mut tp = compiled.type_parser();
        let node = find_native_object_access(t, sf, "Object.keys");
        if !tp.is_native_object_constructor_method_reference(node, "keys")
            || tp.is_native_object_method_reference(node, "keys")
        {
            t.error("inherited constructor roots changed Object.keys namespace");
        }
    });
}

// go test: internal/typeparser/native_object_test.go:232 TestNativeObjectNoLib
// Rust: Go's last check (`len(tp.links.nativeObjectRegistry.instance) != 0 || len(...constructor) != 0`, "noLib
// registry contains canonical members") is left out: the registry's fields are private to `native_object`.
#[test]
fn test_native_object_no_lib() {
    run(|t| {
        t.parallel();
        let source = r"
interface Object { hasOwnProperty(key: string): boolean; }
interface ObjectConstructor { keys(value: object): string[]; prototype: Object; }
declare const Object: ObjectConstructor;
Object.keys;
Object.prototype.hasOwnProperty;
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
        if tp.is_native_object_constructor_method_reference(
            find_native_object_access(t, sf, "Object.keys"),
            "keys",
        ) || tp.is_native_object_method_reference(
            find_native_object_access(t, sf, "Object.prototype.hasOwnProperty"),
            "hasOwnProperty",
        ) {
            t.error("noLib user declarations matched native Object members");
        }
    });
}

// go test: internal/typeparser/native_object_test.go:264 TestNativeObjectCheckerCache
// Rust: a parser borrows its checker mutably, so `tp` and `second` (both `new_type_parser` over the same checker,
// as in Go) are re-created for each use; they share the checker's links. Go compares the cached member pointers;
// they come from one links store, so the values are compared. The member's fields are private to `native_object`,
// so the expected member is compared through its derived `Debug` form.
#[test]
fn test_native_object_checker_cache() {
    run(|t| {
        t.parallel();
        let source = "export {};
declare const custom: { keys: () => void };
Object.keys;
custom.keys;
";
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[("/.src/first.ts", source), ("/.src/second.ts", source)],
        );
        let first = files["/.src/first.ts"];
        let tp_links = Arc::clone(&compiled.type_parser().links);
        if tp_links.lock().native_object_registry.is_some() {
            t.fatal("registry was built before a reference query");
        }
        for access in ["Object.keys", "custom.keys"] {
            let node = find_native_object_access(t, first, access);
            let want = access == "Object.keys";
            let mut tp = compiled.type_parser();
            let got = tp.is_native_object_constructor_method_reference(node, "keys");
            if got != want {
                t.fatal(sprintf!("first parser %s = %v, want %v", access, got, want));
            }
            let symbol = tp.get_symbol_at_location(node);
            let cached = symbol.and_then(|s| tp.links().native_object_member.get(&s).cloned());
            let want_member = if want {
                format!(
                    "NativeObjectMember {{ namespace: {:?}, name: \"keys\" }}",
                    NativeObjectNamespace::Constructor
                )
            } else {
                format!("{:?}", NativeObjectMember::default())
            };
            if cached.as_ref().map(|m| format!("{m:?}")) != Some(want_member) {
                t.fatal(sprintf!("%s classification was not cached", access));
            }
            let mut second = compiled.type_parser();
            let got = second.is_native_object_constructor_method_reference(node, "keys");
            if got != want
                || !Arc::ptr_eq(&second.links, &tp_links)
                || symbol.and_then(|s| second.links().native_object_member.get(&s).cloned())
                    != cached
            {
                t.fatal(sprintf!("second parser did not reuse the %s classification", access));
            }
        }
        let registry = tp_links.lock().native_object_registry.clone();
        let mut second = compiled.type_parser();
        if !second.is_native_object_constructor_method_reference(
            find_native_object_access(t, files["/.src/second.ts"], "Object.keys"),
            "keys",
        ) || !same_arc(second.links().native_object_registry.as_ref(), registry.as_ref())
        {
            t.fatal("second source file did not reuse the canonical registry");
        }
        let (mut other_compiled, other_file) =
            compile_and_get_checker_and_source_file_internal(t, source);
        let mut other = other_compiled.type_parser();
        let first_symbol = compiled
            .type_parser()
            .get_symbol_at_location(find_native_object_access(t, first, "Object.keys"));
        if Arc::ptr_eq(&other.links, &tp_links)
            || other.links().native_object_registry.is_some()
            || first_symbol.is_some_and(|s| other.links().native_object_member.contains_key(&s))
        {
            t.fatal("separate checker inherited native Object cache entries");
        }
        let other_node = find_native_object_access(t, other_file, "Object.keys");
        if !other.is_native_object_constructor_method_reference(other_node, "keys")
            || same_arc(other.links().native_object_registry.as_ref(), registry.as_ref())
            || other.get_symbol_at_location(other_node) == first_symbol
        {
            t.fatal("separate checker did not build its own native Object cache");
        }
    });
}
