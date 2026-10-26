// Left out:
// BenchmarkIsNodeReferenceToModuleExport (benchmarks are ported only for a performance claim).

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, NodeFlags, SourceFile};
use ets_shim::core::{CompilerOptions, JsxEmit, P, Tristate};
use ets_shim::vfs::vfstest::MapFile;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/module_export_reference_test.go:21 moduleExportReferencePackageJSON
const MODULE_EXPORT_REFERENCE_PACKAGE_JSON: &str = r#"{
  "name": "cache-fixture",
  "version": "1.0.0",
  "type": "module",
  "exports": { ".": "./index.d.ts" }
}"#;

// go test: internal/typeparser/module_export_reference_test.go:28 testDescriptorSourceFile
const TEST_DESCRIPTOR_SOURCE_FILE: ModuleDescriptorId =
    ModuleDescriptorId(ModuleDescriptorId::Count.0);
const TEST_DESCRIPTOR_ANY_FILE: ModuleDescriptorId =
    ModuleDescriptorId(ModuleDescriptorId::Count.0 + 1);
const TEST_DESCRIPTOR_OTHER_FILE: ModuleDescriptorId =
    ModuleDescriptorId(ModuleDescriptorId::Count.0 + 2);

// go test: internal/typeparser/module_export_reference_test.go:34 moduleExportReferenceSources
fn module_export_reference_sources() -> Vec<(&'static str, String)> {
    vec![
        (
            "/.src/main.ts",
            "import { Foo as RootFoo, Bar } from \"cache-fixture\"
const rootFoo1 = RootFoo
const rootFoo2 = RootFoo
const rootBar = Bar
const rootFooAlias = RootFoo
const rootFooAlias2 = rootFooAlias
const rootFooAliasUse = rootFooAlias2
let mutableFooAlias = RootFoo
mutableFooAlias = Bar
const mutableFooAliasUse = mutableFooAlias"
                .to_string(),
        ),
        (
            "/packages/consumer/main.ts",
            "import { Foo as NestedFoo } from \"cache-fixture\"
const nestedFoo = NestedFoo"
                .to_string(),
        ),
        (
            "/node_modules/cache-fixture/package.json",
            MODULE_EXPORT_REFERENCE_PACKAGE_JSON.to_string(),
        ),
        (
            "/.src/renamed.ts",
            "import { Base, Renamed } from \"cache-fixture\"
const baseUse = Base
const renamedUse = Renamed"
                .to_string(),
        ),
        (
            "/.src/cjs.ts",
            "import api = require(\"cjs-fixture\")
const runUse = api.run"
                .to_string(),
        ),
        (
            "/node_modules/cache-fixture/index.d.ts",
            "export { Foo, Bar } from \"./source.js\"
export { Base, Renamed } from \"./renamed.js\""
                .to_string(),
        ),
        (
            "/node_modules/cache-fixture/source.d.ts",
            "export declare const Foo: unique symbol
export declare const Bar: unique symbol"
                .to_string(),
        ),
        (
            "/node_modules/cache-fixture/renamed.d.ts",
            "declare const Base: unique symbol
export { Base, Base as Renamed }"
                .to_string(),
        ),
        (
            "/node_modules/cjs-fixture/package.json",
            r#"{ "name": "cjs-fixture", "version": "1.0.0", "types": "./index.d.ts" }"#.to_string(),
        ),
        (
            "/node_modules/cjs-fixture/index.d.ts",
            "declare const api: { run(): void; stop(): void }
export = api"
                .to_string(),
        ),
        (
            "/packages/consumer/node_modules/cache-fixture/package.json",
            MODULE_EXPORT_REFERENCE_PACKAGE_JSON.replacen("1.0.0", "2.0.0", 1),
        ),
        (
            "/packages/consumer/node_modules/cache-fixture/index.d.ts",
            "export { Foo } from \"./source.js\"".to_string(),
        ),
        (
            "/packages/consumer/node_modules/cache-fixture/source.d.ts",
            "export declare const Foo: unique symbol".to_string(),
        ),
    ]
}

// go test: internal/typeparser/module_export_reference_test.go:92 moduleExportReferenceFixture
fn module_export_reference_fixture(t: &T) -> (Compiled, FxHashMap<String, P<SourceFile>>) {
    t.helper();

    let sources = module_export_reference_sources();
    let sources: Vec<(&str, &str)> =
        sources.iter().map(|(path, source)| (*path, source.as_str())).collect();
    compile_and_get_checker_and_source_files_internal(t, &sources)
}

// go test: internal/typeparser/module_export_reference_test.go:99 sourceFileNameMatcher
// Rust: the matcher is leaked to be `'static` like the package-level descriptors' functions; `calls` is shared.
fn source_file_name_matcher(
    suffix: &'static str,
    calls: Option<Arc<AtomicI64>>,
) -> MatchesSourceFile {
    Box::leak(Box::new(move |_: &mut TypeParser<'_>, sf: P<SourceFile>| {
        if let Some(calls) = &calls {
            calls.fetch_add(1, Ordering::SeqCst);
        }
        sf.file_name().ends_with(suffix)
    }))
}

// go test: internal/typeparser/module_export_reference_test.go:69 TestIsNodeReferenceToModuleExport_ConstantAliases
#[test]
fn test_is_node_reference_to_module_export_constant_aliases() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = module_export_reference_fixture(t);
        let mut tp = compiled.type_parser();

        let desc = new_package_source_file_descriptor(
            TEST_DESCRIPTOR_SOURCE_FILE,
            "cache-fixture",
            Some(source_file_name_matcher("/source.d.ts", None)),
        );
        let main = source_files["/.src/main.ts"];
        let constant_alias_declaration = find_identifier_by_text(t, main, "rootFooAlias2", 0);
        let constant_alias = find_identifier_by_text(t, main, "rootFooAlias2", 1);
        let mutable_alias = find_identifier_by_text(t, main, "mutableFooAlias", 2);

        if tp.is_node_reference_to_module_export(constant_alias_declaration, &desc, "Foo") {
            t.fatal("expected a constant alias declaration name not to match the Foo export");
        }
        if !tp.is_node_reference_to_module_export(constant_alias, &desc, "Foo") {
            t.fatal("expected a constant alias chain to match the Foo export");
        }
        if tp.is_node_reference_to_module_export(mutable_alias, &desc, "Foo") {
            t.fatal("expected a mutable alias not to match the Foo export");
        }
    });
}

// go test: internal/typeparser/module_export_reference_test.go:108 TestIsNodeReferenceToModuleExport_AliasesAndReexports
#[test]
fn test_is_node_reference_to_module_export_aliases_and_reexports() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = module_export_reference_fixture(t);
        let mut tp = compiled.type_parser();

        let desc = new_package_source_file_descriptor(
            TEST_DESCRIPTOR_SOURCE_FILE,
            "cache-fixture",
            Some(source_file_name_matcher("/source.d.ts", None)),
        );
        let main = source_files["/.src/main.ts"];
        let root_foo = find_identifier_by_text(t, main, "RootFoo", 1);
        let root_bar = find_identifier_by_text(t, main, "Bar", 1);

        if !tp.is_node_reference_to_module_export(root_foo, &desc, "Foo") {
            t.fatal("expected aliased Foo reference through a reexport to match");
        }
        if tp.is_node_reference_to_module_export(root_foo, &desc, "Bar") {
            t.fatal("expected Foo reference not to match the Bar export");
        }
        if !tp.is_node_reference_to_module_export(root_bar, &desc, "Bar") {
            t.fatal("expected Bar reference through a reexport to match");
        }
    });
}

// go test: internal/typeparser/module_export_reference_test.go:130 TestIsNodeReferenceToModuleExport_DuplicatePackageVersions
#[test]
fn test_is_node_reference_to_module_export_duplicate_package_versions() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = module_export_reference_fixture(t);
        let mut tp = compiled.type_parser();

        let desc =
            new_package_source_file_descriptor(TEST_DESCRIPTOR_ANY_FILE, "cache-fixture", None);
        let root_foo = find_identifier_by_text(t, source_files["/.src/main.ts"], "RootFoo", 1);
        let nested_foo =
            find_identifier_by_text(t, source_files["/packages/consumer/main.ts"], "NestedFoo", 1);

        if !tp.is_node_reference_to_module_export(root_foo, &desc, "Foo") {
            t.fatal("expected Foo from the root package version to match");
        }
        if !tp.is_node_reference_to_module_export(nested_foo, &desc, "Foo") {
            t.fatal("expected Foo from the nested package version to match independently");
        }
        if tp.reference_symbol_at_node(root_foo) == tp.reference_symbol_at_node(nested_foo) {
            t.fatal("expected duplicate package versions to resolve to distinct symbols");
        }
    });
}

// go test: internal/typeparser/module_export_reference_test.go:151 TestIsNodeReferenceToModuleExport_CachesBySymbolDescriptorAndMember
#[test]
fn test_is_node_reference_to_module_export_caches_by_symbol_descriptor_and_member() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = module_export_reference_fixture(t);
        let mut tp = compiled.type_parser();

        let main = source_files["/.src/main.ts"];
        let root_foo1 = find_identifier_by_text(t, main, "RootFoo", 1);
        let root_foo2 = find_identifier_by_text(t, main, "RootFoo", 2);
        let calls = Arc::new(AtomicI64::new(0));
        let matching_desc = new_package_source_file_descriptor(
            TEST_DESCRIPTOR_SOURCE_FILE,
            "cache-fixture",
            Some(source_file_name_matcher("/source.d.ts", Some(Arc::clone(&calls)))),
        );

        if !tp.is_node_reference_to_module_export(root_foo1, &matching_desc, "Foo") {
            t.fatal("expected first Foo reference to match");
        }
        let first_call_count = calls.load(Ordering::SeqCst);
        if !tp.is_node_reference_to_module_export(root_foo2, &matching_desc, "Foo") {
            t.fatal("expected second Foo reference to match");
        }
        if calls.load(Ordering::SeqCst) != first_call_count {
            t.fatal(ets_go::sprintf!(
                "expected resolved-symbol cache hit, matcher calls increased from %d to %d",
                first_call_count,
                calls.load(Ordering::SeqCst)
            ));
        }

        let non_matching_desc = new_package_source_file_descriptor(
            TEST_DESCRIPTOR_OTHER_FILE,
            "cache-fixture",
            Some(source_file_name_matcher("/other.d.ts", None)),
        );
        if tp.is_node_reference_to_module_export(root_foo1, &non_matching_desc, "Foo") {
            t.fatal("expected a different source descriptor not to reuse the cached match");
        }
        if tp.is_node_reference_to_module_export(root_foo1, &matching_desc, "Bar") {
            t.fatal("expected a different member not to reuse the cached Foo match");
        }
    });
}

// go test: internal/typeparser/module_export_reference_test.go:183 TestIsNodeReferenceToModuleExport_SymbolExportedUnderTwoNames
#[test]
fn test_is_node_reference_to_module_export_symbol_exported_under_two_names() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = module_export_reference_fixture(t);
        let mut tp = compiled.type_parser();

        let desc = new_package_source_file_descriptor(
            TEST_DESCRIPTOR_SOURCE_FILE,
            "cache-fixture",
            Some(source_file_name_matcher("/renamed.d.ts", None)),
        );
        let renamed = source_files["/.src/renamed.ts"];
        for node in [
            find_identifier_by_text(t, renamed, "Base", 1),
            find_identifier_by_text(t, renamed, "Renamed", 1),
        ] {
            for member in ["Base", "Renamed"] {
                if !tp.is_node_reference_to_module_export(node, &desc, member) {
                    t.fatal(ets_go::sprintf!(
                        "expected %s to match the %s export of the same declaration",
                        node.text(),
                        member
                    ));
                }
            }
            if tp.is_node_reference_to_module_export(node, &desc, "Foo") {
                t.fatal(ets_go::sprintf!("expected %s not to match the Foo export", node.text()));
            }
        }
    });
}

// go test: internal/typeparser/module_export_reference_test.go:203 TestIsNodeReferenceToModuleExport_ExportEqualsMembers
#[test]
fn test_is_node_reference_to_module_export_export_equals_members() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = module_export_reference_fixture(t);
        let mut tp = compiled.type_parser();

        let desc =
            new_package_source_file_descriptor(TEST_DESCRIPTOR_ANY_FILE, "cjs-fixture", None);
        let run = find_identifier_by_text(t, source_files["/.src/cjs.ts"], "run", 0);

        if !tp.is_node_reference_to_module_export(run, &desc, "run") {
            t.fatal("expected a member of an export= value to match its name");
        }
        if tp.is_node_reference_to_module_export(run, &desc, "stop") {
            t.fatal("expected run not to match the stop member");
        }
    });
}

// go test: internal/typeparser/module_export_reference_test.go:224 compileFixturesWithJS
// compileFixturesWithJS compiles every .ts, .tsx and .js source outside
// node_modules as a root file, with JS checking and JSX enabled. When version
// is not empty the bundled Effect package is mounted too. Source files are
// returned sorted by name.
//
// Rust: Go's empty version is `None`.
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "Go: strings.HasSuffix(path, \".ts\")"
)]
pub(crate) fn compile_fixtures_with_js(
    t: &T,
    version: Option<EffectVersion>,
    sources: &[(&str, &str)],
) -> (Compiled, Vec<P<SourceFile>>) {
    t.helper();

    let mut testfs = TestFS::default();
    if let Some(version) = version
        && let Err(err) = mount_effect(version, &mut testfs)
    {
        t.fatal(ets_go::sprintf!("failed to mount effect %s: %v", version, err));
    }
    let mut file_names: Vec<String> = Vec::new();
    for &(path, source) in sources {
        testfs.insert(path.to_string(), MapFile::from(source));
        if !path.contains("/node_modules/")
            && (path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".js"))
        {
            file_names.push(path.to_string());
        }
    }
    file_names.sort();

    let compiler_options = CompilerOptions {
        allow_js: Tristate::True,
        check_js: Tristate::True,
        jsx: JsxEmit::PRESERVE,
        ..test_compiler_options()
    };
    let compiled = new_compiled(testfs, compiler_options, file_names.clone());
    let program = compiled.program;
    let mut source_files = Vec::with_capacity(file_names.len());
    for file_name in &file_names {
        let Some(sf) = program.get_source_file(file_name) else {
            t.fatal(ets_go::sprintf!("failed to get source file %s", file_name));
        };
        source_files.push(sf);
    }
    (compiled, source_files)
}

/// Rust-only: Go's `%v` of an `ast.Kind` (its stringer, `KindIdentifier`).
pub(crate) fn kind_string(kind: Kind) -> String {
    format!("Kind{kind:?}")
}

// go test: internal/typeparser/module_export_reference_test.go:276 TestReferenceSymbolAtNode_MatchesUngatedComputation
#[test]
fn test_reference_symbol_at_node_matches_ungated_computation() {
    run(|t| {
        t.parallel();

        let mut sources = module_export_reference_sources();
        sources.push((
            "/.src/syntax.ts",
            r#"import * as Module from "cache-fixture"
import { Foo as RenamedFoo, "Bar" as QuotedBar } from "cache-fixture"
export { RenamedFoo as ReexportedFoo }
const key = Symbol()
class Base { value = Module.Foo }
class Derived extends Base {
  #private = Module.Foo;
  "literal" = Module.Foo;
  0 = Module.Foo;
  1n = Module.Foo;
  ["computed"] = Module.Foo;
  [key] = Module.Foo;
  constructor() { super() }
  method(): this {
    super.value
    this.#private
    #private in this
    return this
  }
}
type Imported = typeof import("cache-fixture")
type ImportedMember = import("cache-fixture").Foo
type Qualified = typeof Module.Foo
type Indexed = { a: 1 }["a"]
const object = { "literal": Module.Foo, 0: Module.Foo, [`template`]: 1 }
object["literal"]
object[0]
object[`literal`]
delete (object as any).literal
const { literal: alias, ...rest } = object
const [first] = [alias]
const arrow = () => alias
const big = 1n
new Derived() instanceof Base
function target() { return new.target }
const metadata = import.meta
const loaded = import("cache-fixture")
label: for (const item of [first]) { if (item) break label; else continue label }
switch (big) { case 1n: break; default: }
try { throw QuotedBar } catch { } finally { }
enum Color { Red, "Green" }
namespace Space { export const inner = Color.Red }
declare module "ambient" { export const value: number }
export default class Default {}"#
                .to_string(),
        ));
        sources.push((
            "/.src/docs.ts",
            r#"import * as Module from "cache-fixture"
/**
 * @template T
 * @param {T} value
 * @returns {T}
 * @see Module.Foo
 * @see {@link Module.Bar}
 */
export function identity<T>(value: T): T { return value }
/** @typedef {{ value: typeof Module.Foo }} Options */
export const options = { value: Module.Foo }"#
                .to_string(),
        ));
        sources
            .push(("/.src/export-equals.ts", "const api = { run() {} }; export = api".to_string()));
        sources.push((
            "/.src/export-default.ts",
            "export default function () { return 1 }".to_string(),
        ));
        sources.push((
            "/.src/jsdoc.js",
            r#"// @ts-check
import * as Module from "cache-fixture"
/** @import { Foo } from "cache-fixture" */
/** @typedef {{ a: number, foo: typeof Module.Foo }} Shape */
/** @callback Mapper
 * @param {number} n
 * @returns {string}
 */
/**
 * @template {Shape} T
 * @param {T} value
 * @param {Mapper} [map]
 * @returns {T["a"]}
 */
export function pick(value, map) { return value.a }
/** @type {Shape} */
export const shape = { a: 1, foo: Module.Foo }
export const cast = /** @type {number} */ (/** @type {unknown} */ (shape.a))
export const sat = /** @satisfies {Shape} */ ({ a: 2, foo: Module.Foo })
export class Box {
  /** @type {number} */
  size = 1
  /** @param {number} n @this {Box} */
  grow(n) { this.size += n; return this }
}
/** @enum {string} */
export const Mode = { A: "a" }
module.exports.extra = 1
exports["other"] = 2"#
                .to_string(),
        ));
        sources.push((
            "/.src/view.tsx",
            r##"declare global { namespace JSX { interface IntrinsicElements { div: any; "svg:rect": { "xlink:href"?: string } } } }
const Comp = (props: { label: string }) => <div data-label={props.label}>{props.label}</div>
export const view = <><Comp label="x" /><svg:rect xlink:href="#a" /></>"##
                .to_string(),
        ));

        let sources: Vec<(&str, &str)> =
            sources.iter().map(|(path, source)| (*path, source.as_str())).collect();
        let (mut compiled, source_files) = compile_fixtures_with_js(t, None, &sources);
        let mut tp = compiled.type_parser();

        struct Counts {
            kinds: FxHashMap<Kind, i64>,
            symbols: i64,
            uncached: i64,
            jsdoc: i64,
        }
        let mut counts = Counts { kinds: FxHashMap::default(), symbols: 0, uncached: 0, jsdoc: 0 };
        fn visit(
            t: &T,
            tp: &mut TypeParser<'_>,
            sf: P<SourceFile>,
            node: P<Node>,
            counts: &mut Counts,
        ) -> bool {
            *counts.kinds.entry(node.kind()).or_default() += 1;
            let symbol = tp.get_symbol_at_location(node);
            let want = tp.reference_symbol_from(node, symbol);
            for i in 0..2_i64 {
                let got = tp.reference_symbol_at_node(node);
                if got != want {
                    t.error(ets_go::sprintf!(
                        "%s:%d %s: lookup %d got symbol %v, want %v",
                        sf.file_name(),
                        node_pos(node),
                        kind_string(node.kind()),
                        i,
                        got.map(ets_shim::ast::symbol_name),
                        want.map(ets_shim::ast::symbol_name)
                    ));
                }
            }
            if want.is_some() {
                counts.symbols += 1;
            }
            if node.flags().intersects(NodeFlags::JSDoc) {
                counts.jsdoc += 1;
                if !tp.links().reference_symbol.contains_key(&node) {
                    t.error(ets_go::sprintf!(
                        "%s:%d %s: JSDoc node skipped the ungated path",
                        sf.file_name(),
                        node_pos(node),
                        kind_string(node.kind())
                    ));
                }
            } else if !can_have_symbol_at_location(node.kind()) {
                counts.uncached += 1;
                if tp.links().reference_symbol.contains_key(&node) {
                    t.error(ets_go::sprintf!(
                        "%s:%d %s: gated node has a cache entry",
                        sf.file_name(),
                        node_pos(node),
                        kind_string(node.kind())
                    ));
                }
            }
            ets_shim::ast::for_each_child_and_jsdoc(node, sf.get(), &mut |child| {
                visit(t, tp, sf, child, counts)
            });
            false
        }
        for &sf in &source_files {
            if !sources.iter().any(|(path, _)| *path == sf.file_name()) {
                continue;
            }
            visit(t, &mut tp, sf, sf.as_node(), &mut counts);
        }
        if counts.symbols == 0 || counts.uncached == 0 || counts.jsdoc == 0 {
            t.fatal(ets_go::sprintf!(
                "expected symbols, gated nodes and JSDoc nodes, got %d, %d, %d",
                counts.symbols,
                counts.uncached,
                counts.jsdoc
            ));
        }
        for kind in [
            Kind::SourceFile,
            Kind::Identifier,
            Kind::PrivateIdentifier,
            Kind::StringLiteral,
            Kind::NumericLiteral,
            Kind::BigIntLiteral,
            Kind::NoSubstitutionTemplateLiteral,
            Kind::ComputedPropertyName,
            Kind::PropertyAccessExpression,
            Kind::ElementAccessExpression,
            Kind::QualifiedName,
            Kind::ThisKeyword,
            Kind::ThisType,
            Kind::SuperKeyword,
            Kind::DefaultKeyword,
            Kind::ExportKeyword,
            Kind::EqualsGreaterThanToken,
            Kind::ImportType,
            Kind::MetaProperty,
            Kind::JsxNamespacedName,
            Kind::ExportAssignment,
            Kind::BinaryExpression,
            Kind::JSDoc,
            Kind::JSDocParameterTag,
            Kind::JSDocReturnTag,
            Kind::JSDocTypeTag,
            Kind::JSDocTemplateTag,
            Kind::JSDocTypedefTag,
            Kind::JSDocCallbackTag,
            Kind::JSDocImportTag,
            Kind::Block,
            Kind::CallExpression,
            Kind::VariableStatement,
            Kind::LabeledStatement,
            Kind::CaseClause,
            Kind::CatchClause,
            Kind::HeritageClause,
            Kind::JsxAttribute,
        ] {
            if counts.kinds.get(&kind).copied().unwrap_or_default() == 0 {
                t.error(ets_go::sprintf!("fixture did not exercise %s", kind_string(kind)));
            }
        }
    });
}

/// Rust-only: Go's `node.Pos()` in a test message (`-1` for a synthesized node).
pub(crate) fn node_pos(node: P<Node>) -> i64 {
    node.pos().source_offset().map_or(-1, i64::from)
}

// go test: internal/typeparser/module_export_reference_test.go:430 TestSymbolHasDeclarationInModuleCached_DeclarationCounts
#[test]
fn test_symbol_has_declaration_in_module_cached_declaration_counts() {
    run(|t| {
        t.parallel();

        let mut sources = module_export_reference_sources();
        let mut append = |path: &str, text: &str| {
            let entry = sources.iter_mut().find(|(p, _)| *p == path).expect("fixture file");
            entry.1.push_str(text);
        };
        append(
            "/node_modules/cache-fixture/source.d.ts",
            "
export interface Merged { first: string }
export interface Merged { second: number }",
        );
        append(
            "/node_modules/cache-fixture/index.d.ts",
            "
export type { Merged } from \"./source.js\"",
        );
        append(
            "/.src/main.ts",
            "
import type { Merged } from \"cache-fixture\"
declare const merged: Merged",
        );
        let sources: Vec<(&str, &str)> =
            sources.iter().map(|(path, source)| (*path, source.as_str())).collect();
        let (mut compiled, source_files) =
            compile_and_get_checker_and_source_files_internal(t, &sources);
        let mut tp = compiled.type_parser();

        for (name, occurrence, declarations) in [("RootFoo", 1, 1_usize), ("Merged", 1, 2)] {
            let sym = tp.reference_symbol_at_node(find_identifier_by_text(
                t,
                source_files["/.src/main.ts"],
                name,
                occurrence,
            ));
            let Some(sym) = sym.filter(|sym| sym.declarations().len() == declarations) else {
                t.fatal(ets_go::sprintf!(
                    "expected %d declarations for %s, got %v",
                    declarations as i64,
                    name,
                    sym.map(ets_shim::ast::symbol_name)
                ));
            };
            for desc in [
                new_package_source_file_descriptor(
                    TEST_DESCRIPTOR_SOURCE_FILE,
                    "cache-fixture",
                    Some(source_file_name_matcher("/source.d.ts", None)),
                ),
                new_package_source_file_descriptor(TEST_DESCRIPTOR_ANY_FILE, "cache-fixture", None),
                new_package_source_file_descriptor(
                    TEST_DESCRIPTOR_OTHER_FILE,
                    "cache-fixture",
                    Some(source_file_name_matcher("/other.d.ts", None)),
                ),
            ] {
                let want = desc.id != TEST_DESCRIPTOR_OTHER_FILE;
                for _ in 0..2 {
                    let got = tp.symbol_has_declaration_in_module_cached(sym, &desc);
                    if got != want {
                        t.error(ets_go::sprintf!(
                            "%s descriptor %d: got %v, want %v",
                            name,
                            i64::from(desc.id.0),
                            got,
                            want
                        ));
                    }
                }
            }
            let cached = tp.links().module_export_filter.contains_key(&sym);
            if cached != (declarations > 1) {
                t.error(ets_go::sprintf!(
                    "%s: per-symbol filter cached = %v for %d declarations",
                    name,
                    cached,
                    declarations as i64
                ));
            }
        }
    });
}
