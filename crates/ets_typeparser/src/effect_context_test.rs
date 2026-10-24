// Ported from effect-tsgo's internal/typeparser/effect_context_test.go (see helpers_test.rs).

use ets_go::fmt::GoValue;
use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, NodeFlags, SourceFile};
use ets_shim::core::P;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::helpers_test::*;
use crate::module_export_reference_test::{compile_fixtures_with_js, kind_string, node_pos};
use crate::*;

// go test: internal/typeparser/effect_context_test.go:15 effectContextFixtureTS
pub(crate) const EFFECT_CONTEXT_FIXTURE_TS: &str = r#"import { Effect, pipe } from "effect"
import * as Eff from "effect/Effect"
import { gen, sync as syncAlias } from "effect/Effect"

class Service {
  readonly n = 1
  method() {
    return Effect.gen(this, function*() {
      return this.n
    })
  }
}

export const nested = Effect.gen(function*() {
  const a = yield* Effect.succeed(1)
  const inner = Effect.gen(function*() {
    const b: number = yield* Effect.succeed(a)
    try {
      yield* Effect.fail("x")
    } catch (e) {
      return e
    }
    return b
  })
  const plain = function* () {
    yield 1
    yield* [2, 3]
    return Effect.succeed(1)
  }
  const arrow = (x: number = a): number => x + 1
  function declared(this: unknown, y = () => Effect.succeed(1)) {
    return y()
  }
  const obj = {
    method() { return Effect.succeed(1) },
    get value() { return 2 },
    set value(v: number) {},
    *gen() { yield 1 },
    prop: () => Effect.sync(() => new Date())
  }
  class Local {
    field = Effect.sync(() => Date.now())
    constructor(readonly z: number = 1) {}
    static { Date.now() }
  }
  // @effect-diagnostics-next-line returnEffectInGen:off
  return yield* inner.pipe(Effect.map((n) => n + 1))
}).pipe(Effect.withSpan("nested"))

export const fnTraced = Effect.fn("fnTraced")(function*(x: number) {
  yield* Effect.log(x)
  const nestedFn = Effect.fn(function*() { return yield* Effect.succeed(x) })
  return x
}, Effect.map((n) => n + 1))

export const fnDirect = Effect.fn(function*(x: number) {
  return yield* Effect.succeed(x)
})

export const fnUntraced = Effect.fnUntraced(function*() {
  return yield* Effect.succeed(1)
})

export const fnArrow = Effect.fn("arrow")((x: number) => Effect.succeed(x))

export const fnOptions = Effect.fn({ self: 1 } as any, function*(this: unknown) {
  const self = this
  return yield* Effect.succeed(self)
})

export class WithAccessors {
  private readonly value = 1
  get eff() {
    return Effect.gen(this, function*() { return this.value })
  }
  set eff(_: unknown) {
    Effect.runSync(Effect.sync(() => Date.now()))
  }
  static make = Effect.fn("WithAccessors.make")(function*() {
    class Inner { m() { return Date.now() } }
    try {
      return new Inner().m()
    } catch {
      return yield* Effect.die("x")
    } finally {
      yield* Effect.log("done")
    }
  })
}

export const thunks = [
  Effect.sync(() => Date.now()),
  Effect.sync(function() { return Date.now() }),
  Effect.sync((() => Date.now())),
  Effect.sync((((() => Date.now())))),
  Effect.sync((() => Date.now()) as () => number),
  Effect.sync((() => Date.now())!),
  Effect.sync((() => Date.now()) satisfies () => number),
  Effect.sync(<() => number>(() => Date.now())),
  Effect.sync(async () => Date.now()),
  Effect.promise(async () => Date.now()),
  Effect.promise(async function*() { yield 1 } as any),
  Effect.suspend(() => Effect.succeed(Date.now())),
  Effect.async<number>((resume) => { resume(Effect.succeed(Date.now())) }),
  Effect.callback<number>((resume) => { resume(Effect.succeed(Date.now())) }),
  Effect.try(() => JSON.parse("1")),
  Effect.try(function() { return JSON.parse("1") }),
  Effect.try({ try: () => JSON.parse("1"), catch: (e) => e }),
  Effect.try(({ try: () => JSON.parse("1"), catch: (e) => e })),
  Effect.try({ try: (() => JSON.parse("1")), catch: (e) => e }),
  Effect.try({ try: () => 1, try: () => 2, catch: (e) => e } as any),
  Effect.try({ "try": () => 1, catch: (e: unknown) => e } as any),
  Effect.try({ try: { try: () => 1 }, catch: (e: unknown) => e } as any),
  Effect.tryPromise({ try: async () => 1, catch: (e) => e }),
  Effect.tryPromise({ try() { return Promise.resolve(1) }, catch: (e) => e }),
  Effect.sync(Date.now),
  Effect.sync(true ? () => 1 : () => 2),
  Effect.sync(...[() => 1]),
  Eff.sync(() => Date.now()),
  syncAlias(() => Date.now()),
  gen(function*() { return yield* Eff.sync(() => 1) }),
  Effect.gen({ self: 1 }, function*() { return yield* Effect.succeed(1) }),
  Effect.sync(() => Effect.runSync(Effect.gen(function*() { return 1 }))),
  Effect.sync(() => () => Date.now()),
  Effect.gen(function*() {
    const x = yield* Effect.sync(() => () => Date.now())
    const y = yield* Effect.try({ try: () => Effect.gen(function*() { return 1 }), catch: (e) => e })
    return Effect.sync(() => { const f = () => Date.now(); return f() })
  }),
]

export const piped = pipe(
  Effect.succeed(1),
  Effect.flatMap((n) => Effect.gen(function*() { return n }))
)

function* notEffect(): Generator<number> {
  yield* notEffect()
  return Effect.gen(function*() {})
}
"#;

// go test: internal/typeparser/effect_context_test.go:157 effectContextFixtureJS
pub(crate) const EFFECT_CONTEXT_FIXTURE_JS: &str = r#"// @ts-check
import { Effect } from "effect"

/** @typedef {{ a: number }} Shape */

/**
 * @template T
 * @param {T} value
 * @returns {Effect.Effect<T>}
 */
export function lift(value) {
  return Effect.gen(function* () {
    /** @type {number} */
    const n = yield* Effect.succeed(1)
    const cast = /** @type {number} */ (n)
    const sat = /** @satisfies {number} */ (n)
    return value
  })
}

/** @type {(x: number) => Effect.Effect<number>} */
export const typed = (x) => Effect.sync(() => /** @type {number} */ (Date.now()) + x)

export const castThunk = Effect.sync(/** @type {() => number} */ (() => Date.now()))

export const tried = Effect.try({ try: /** @type {() => number} */ (() => 1), catch: (e) => e })

/**
 * @overload
 * @param {string} a
 * @returns {void}
 */
/**
 * @param {string | number} a
 */
export function over(a) {}

export class K {
  /** @type {number} */
  x = 1
  /** @param {number} y */
  m(y) { return Effect.gen(this, function* () { return y }) }
}
"#;

/// Rust-only: the state Go's `walk` closure in `effectContextReference` captures, Go's `core.LinkStore`s as maps
/// (`Get` inserts the zero value, `Has` is `contains_key`, `TryGet` is `get`).
struct EffectContextReferenceWalker<'a, 'c> {
    tp: &'a mut TypeParser<'c>,
    flags_store: FxHashMap<P<Node>, EffectContextFlags>,
    generator_store: FxHashMap<P<Node>, Option<P<Node>>>,
    pending_enable_flags: FxHashMap<P<Node>, EffectContextFlags>,
    pending_disable_flags: FxHashMap<P<Node>, EffectContextFlags>,
    pending_flags_mask: EffectContextFlags,
    function_scope_reset_flags: EffectContextFlags,
}

/// Rust-only: the two lookups `effectContextReference` returns (Go returns them as closures).
struct EffectContextReference {
    flags_store: FxHashMap<P<Node>, EffectContextFlags>,
    generator_store: FxHashMap<P<Node>, Option<P<Node>>>,
}

// go test: internal/typeparser/effect_context_test.go:205 effectContextReference
// effectContextReference is the analysis that stored an entry for every node.
// It is kept here to prove that storing only the entries that differ from the
// parent's answers identically for every node.
fn effect_context_reference(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> EffectContextReference {
    let pending_flags_mask = EffectContextFlags::PendingNextFunctionIsEffectThunk
        | EffectContextFlags::PendingNextObjectTryPropertyIsEffectThunk;
    let function_scope_reset_flags = EffectContextFlags::CanYieldEffect
        | EffectContextFlags::InEffectConstructorThunk
        | pending_flags_mask;
    let mut walker = EffectContextReferenceWalker {
        tp,
        flags_store: FxHashMap::default(),
        generator_store: FxHashMap::default(),
        pending_enable_flags: FxHashMap::default(),
        pending_disable_flags: FxHashMap::default(),
        pending_flags_mask,
        function_scope_reset_flags,
    };
    walker.walk(sf.as_node());
    EffectContextReference {
        flags_store: walker.flags_store,
        generator_store: walker.generator_store,
    }
}

impl EffectContextReferenceWalker<'_, '_> {
    // go test: internal/typeparser/effect_context_test.go:205 effectContextReference
    // Rust: Go's `setPendingEnableFlags` closure.
    fn set_pending_enable_flags(&mut self, node: Option<P<Node>>, flags: EffectContextFlags) {
        let Some(node) = node else {
            return;
        };
        if flags == EffectContextFlags::None {
            return;
        }
        *self.pending_enable_flags.entry(node).or_default() |= flags;
    }

    // go test: internal/typeparser/effect_context_test.go:205 effectContextReference
    // Rust: Go's `setPendingDisableFlags` closure.
    fn set_pending_disable_flags(&mut self, node: Option<P<Node>>, flags: EffectContextFlags) {
        let Some(node) = node else {
            return;
        };
        if flags == EffectContextFlags::None {
            return;
        }
        *self.pending_disable_flags.entry(node).or_default() |= flags;
    }

    // go test: internal/typeparser/effect_context_test.go:205 effectContextReference
    // Rust: Go's `walk` closure.
    fn walk(&mut self, node: P<Node>) -> bool {
        if let Some(parent) = node.parent() {
            let parent_flags = self.flags_store.get(&parent).copied().unwrap_or_default();
            self.flags_store.insert(node, parent_flags);
            if !self.generator_store.contains_key(&node) {
                let parent_generator = self.generator_store.get(&parent).copied().flatten();
                self.generator_store.insert(node, parent_generator);
            }
        } else {
            self.flags_store.insert(node, EffectContextFlags::None);
        }

        if let Some(&disable) = self.pending_disable_flags.get(&node) {
            let flags = self.flags_store.entry(node).or_default();
            *flags = *flags & !disable;
        }

        if let Some(&enable) = self.pending_enable_flags.get(&node) {
            *self.flags_store.entry(node).or_default() |= enable;
        }

        let pending_flags_mask = self.pending_flags_mask;
        let flags = self.flags_store[&node];
        if flags.intersects(EffectContextFlags::PendingNextFunctionIsEffectThunk)
            && (node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression)
        {
            if let Some(body) = node.body() {
                self.set_pending_enable_flags(
                    Some(body),
                    EffectContextFlags::InEffectConstructorThunk,
                );
                self.set_pending_disable_flags(Some(body), pending_flags_mask);
            }
        } else if flags.intersects(EffectContextFlags::PendingNextObjectTryPropertyIsEffectThunk)
            && node.kind() == Kind::ObjectLiteralExpression
        {
            node.for_each_child(&mut |child| {
                // Go's `resetPendingFlags`.
                self.set_pending_disable_flags(Some(child), pending_flags_mask);
                false
            });

            let obj = node.as_object_literal_expression();
            for &prop in obj.properties().nodes() {
                if prop.kind() != Kind::PropertyAssignment {
                    continue;
                }
                // Rust: a property assignment always has a name and an initializer (Go's nil checks drop).
                let assignment = prop.as_property_assignment();
                if assignment.name().text() != "try" {
                    continue;
                }
                self.set_pending_enable_flags(
                    Some(assignment.initializer()),
                    EffectContextFlags::PendingNextFunctionIsEffectThunk,
                );
            }
        } else if let Some(expr) = transparent_pending_expression(node) {
            self.set_pending_enable_flags(Some(expr), flags & pending_flags_mask);
        } else if flags.intersects(pending_flags_mask) {
            node.for_each_child(&mut |child| {
                // Go's `resetPendingFlags`.
                self.set_pending_disable_flags(Some(child), pending_flags_mask);
                false
            });
        }

        if let Some(effect_gen) = self.tp.effect_gen_call(node) {
            let body_node = effect_gen.body;
            self.set_pending_enable_flags(body_node, EffectContextFlags::CanYieldEffect);
            if let Some(body_node) = body_node {
                self.generator_store.insert(body_node, Some(effect_gen.generator_function));
            }
        } else if let Some(effect_fn) = self.tp.effect_fn_call(node)
            && effect_fn.is_generator()
        {
            let body = effect_fn.body();
            let gen_fn = effect_fn.generator_function();
            if let (Some(body_node), Some(gen_fn)) = (body, gen_fn) {
                self.set_pending_enable_flags(Some(body_node), EffectContextFlags::CanYieldEffect);
                self.generator_store.insert(body_node, Some(gen_fn));
            }
        }

        if node.kind() == Kind::CallExpression {
            let call = node.as_call_expression();
            let arguments = call.arguments().nodes();
            if !arguments.is_empty() {
                let effect_thunk_arg = arguments[0];
                let expression = call.expression();
                if self.tp.is_node_reference_to_effect_module_api(expression, "sync")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "promise")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "callback")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "suspend")
                {
                    self.set_pending_enable_flags(
                        Some(effect_thunk_arg),
                        EffectContextFlags::PendingNextFunctionIsEffectThunk,
                    );
                } else if self.tp.is_node_reference_to_effect_module_api(expression, "try")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "tryPromise")
                {
                    self.set_pending_enable_flags(
                        Some(effect_thunk_arg),
                        EffectContextFlags::PendingNextFunctionIsEffectThunk
                            | EffectContextFlags::PendingNextObjectTryPropertyIsEffectThunk,
                    );
                }
            }
        }

        if ets_shim::ast::is_function_like(Some(node)) {
            let function_scope_reset_flags = self.function_scope_reset_flags;
            node.for_each_child(&mut |child| {
                // Go's `resetChildFunctionScopeFlags`.
                self.set_pending_disable_flags(Some(child), function_scope_reset_flags);
                false
            });
        }

        if !self.flags_store[&node].intersects(EffectContextFlags::CanYieldEffect) {
            self.generator_store.insert(node, None);
        }

        node.for_each_child(&mut |child| self.walk(child));
        false
    }
}

// go test: internal/typeparser/effect_context_test.go:205 effectContextReference
// Rust: Go's `transparentPendingExpression` closure (it captures nothing).
fn transparent_pending_expression(node: P<Node>) -> Option<P<Node>> {
    match node.kind() {
        Kind::ParenthesizedExpression
        | Kind::SatisfiesExpression
        | Kind::AsExpression
        | Kind::NonNullExpression
        | Kind::TypeAssertionExpression => node.expression(),
        _ => None,
    }
}

impl EffectContextReference {
    // go test: internal/typeparser/effect_context_test.go:205 effectContextReference
    // Rust: Go's `flagsOf` closure.
    fn flags_of(&self, node: P<Node>) -> EffectContextFlags {
        let mut current = Some(node);
        while let Some(c) = current {
            if let Some(&flags) = self.flags_store.get(&c) {
                return flags & EffectContextFlags::InEffect;
            }
            current = c.parent();
        }
        EffectContextFlags::None
    }

    // go test: internal/typeparser/effect_context_test.go:205 effectContextReference
    // Rust: Go's `generatorOf` closure.
    fn generator_of(&self, node: P<Node>) -> Option<P<Node>> {
        let mut current = Some(node);
        while let Some(c) = current {
            if let Some(&gen_fn) = self.generator_store.get(&c) {
                return gen_fn;
            }
            current = c.parent();
        }
        None
    }
}

// go test: internal/typeparser/effect_context_test.go:368 allEffectContextNodes
// allEffectContextNodes returns every node reachable from the source file,
// JSDoc nodes included, which the analysis does not visit.
fn all_effect_context_nodes(sf: P<SourceFile>) -> Vec<P<Node>> {
    fn visit(sf: P<SourceFile>, node: P<Node>, nodes: &mut Vec<P<Node>>) -> bool {
        nodes.push(node);
        ets_shim::ast::for_each_child_and_jsdoc(node, sf.get(), &mut |child| {
            visit(sf, child, nodes)
        });
        false
    }
    let mut nodes = Vec::new();
    visit(sf, sf.as_node(), &mut nodes);
    nodes
}

// go test: internal/typeparser/effect_context_test.go:380 effectContextComparison
#[derive(Default)]
struct EffectContextComparison {
    nodes: i64,
    stored: i64,
    can_yield: i64,
    in_thunk: i64,
    jsdoc: i64,
    generators: FxHashSet<P<Node>>,
}

/// Rust-only: Go's `%p` of a `*ast.FunctionExpression` (nil prints `0x0`).
fn function_expression_pointer(node: Option<P<Node>>) -> GoValue<'static> {
    GoValue::Pointer {
        ty: "*ast.FunctionExpression".into(),
        addr: node.map_or(0, P::addr),
        elem: None,
    }
}

// go test: internal/typeparser/effect_context_test.go:389 compareEffectContext
// compareEffectContext checks every node of every file against the reference
// analysis. Odd files are queried bottom-up and every third file asks for the
// generator function first, so the first query of a file is often neither the
// source file nor a flags query.
fn compare_effect_context(
    t: &T,
    tp: &mut TypeParser<'_>,
    source_files: &[P<SourceFile>],
) -> EffectContextComparison {
    t.helper();
    let mut result = EffectContextComparison::default();
    for (i, &sf) in source_files.iter().enumerate() {
        let mut nodes = all_effect_context_nodes(sf);
        if i % 2 == 1 {
            nodes.reverse();
        }
        let mut want: Option<EffectContextReference> = None;
        let mut mismatches = 0;
        for &node in &nodes {
            let got_flags;
            let got_generator;
            if i % 3 == 1 {
                got_generator = tp.get_effect_yield_generator_function(node);
                got_flags = tp.get_effect_context_flags(node);
            } else {
                got_flags = tp.get_effect_context_flags(node);
                got_generator = tp.get_effect_yield_generator_function(node);
            }
            // after the first query, so the file is analyzed lazily by the getters
            let want = want.get_or_insert_with(|| effect_context_reference(tp, sf));
            let want_flags = want.flags_of(node);
            if got_flags != want_flags || got_generator != want.generator_of(node) {
                mismatches += 1;
                if mismatches <= 10 {
                    t.error(sprintf!(
                        "%s: %v at %d: flags %d, want %d; generator %p, want %p",
                        sf.file_name(),
                        kind_string(node.kind()),
                        node_pos(node),
                        got_flags.0,
                        want_flags.0,
                        function_expression_pointer(got_generator),
                        function_expression_pointer(want.generator_of(node))
                    ));
                }
            }
            result.nodes += 1;
            if tp.links().effect_context_flags.contains_key(&node)
                || tp.links().effect_yield_generator_function.contains_key(&node)
            {
                result.stored += 1;
            }
            if node.flags().intersects(NodeFlags::JSDoc) {
                result.jsdoc += 1;
            }
            if got_flags.intersects(EffectContextFlags::CanYieldEffect) {
                result.can_yield += 1;
            }
            if got_flags.intersects(EffectContextFlags::InEffectConstructorThunk) {
                result.in_thunk += 1;
            }
            if let Some(got_generator) = got_generator {
                result.generators.insert(got_generator);
            }
        }
        if !tp.links().effect_context_flags.contains_key(&sf.as_node())
            || !tp.links().effect_yield_generator_function.contains_key(&sf.as_node())
        {
            t.error(sprintf!("%s: analyzed source file has no entry", sf.file_name()));
        }
    }
    result
}

// go test: internal/typeparser/effect_context_test.go:444 TestEffectContextMatchesPerNodeAnalysis
#[test]
fn test_effect_context_matches_per_node_analysis() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let (mut compiled, source_files) = compile_fixtures_with_js(
                    t,
                    Some(version),
                    &[
                        ("/.src/fixture.ts", EFFECT_CONTEXT_FIXTURE_TS),
                        ("/.src/fixture.js", EFFECT_CONTEXT_FIXTURE_JS),
                    ],
                );
                let mut tp = compiled.type_parser();
                let result = compare_effect_context(t, &mut tp, &source_files);
                t.log(sprintf!(
                    "%d nodes, %d with an entry, %d JSDoc, %d can yield, %d in thunks, %d generator functions",
                    result.nodes,
                    result.stored,
                    result.jsdoc,
                    result.can_yield,
                    result.in_thunk,
                    result.generators.len() as i64
                ));
                if result.can_yield == 0
                    || result.in_thunk == 0
                    || result.jsdoc == 0
                    || result.generators.len() < 10
                {
                    t.fatal("fixture does not exercise the analysis");
                }
                if result.stored * 4 > result.nodes {
                    t.error(sprintf!(
                        "%d of %d nodes have an entry, want at most a quarter",
                        result.stored,
                        result.nodes
                    ));
                }
            });
        }
    });
}

// go test: internal/typeparser/effect_context_test.go:466 TestEffectContextMatchesPerNodeAnalysisOnRuleFixtures
#[test]
fn test_effect_context_matches_per_node_analysis_on_rule_fixtures() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                // Rust: Go's `filepath.Glob(root/<version>*)` (sorted names), from the package directory's
                // `../../testdata/tests`.
                let root =
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/tests");
                let mut dirs: Vec<std::path::PathBuf> = match std::fs::read_dir(&root) {
                    Ok(entries) => entries
                        .filter_map(Result::ok)
                        .filter(|entry| entry.file_name().to_string_lossy().starts_with(version))
                        .map(|entry| entry.path())
                        .collect(),
                    Err(err) => t.fatal(sprintf!(
                        "no fixture directories for %s: %v",
                        version,
                        err.to_string()
                    )),
                };
                dirs.sort();
                if dirs.is_empty() {
                    t.fatal(sprintf!(
                        "no fixture directories for %s: %v",
                        version,
                        None::<ets_go::Error>
                    ));
                }
                let mut sources: FxHashMap<String, String> = FxHashMap::default();
                for dir in &dirs {
                    let entries = match std::fs::read_dir(dir) {
                        Ok(entries) => entries,
                        Err(err) => t.fatal(sprintf!(
                            "read %s: %v",
                            dir.display().to_string(),
                            err.to_string()
                        )),
                    };
                    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
                    entries.sort_by_key(std::fs::DirEntry::file_name);
                    for entry in entries {
                        let entry_name = entry.file_name().to_string_lossy().into_owned();
                        #[expect(
                            clippy::case_sensitive_file_extension_comparisons,
                            reason = "Go: strings.HasSuffix(entry.Name(), \".ts\")"
                        )]
                        let is_ts = entry_name.ends_with(".ts");
                        if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) || !is_ts {
                            continue;
                        }
                        let content = match std::fs::read_to_string(entry.path()) {
                            Ok(content) => content,
                            Err(err) => {
                                t.fatal(sprintf!("read %s: %v", entry_name, err.to_string()))
                            }
                        };
                        let base = dir
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        sources.insert(format!("/.src/{base}/{entry_name}"), content);
                    }
                }
                let sources: Vec<(&str, &str)> =
                    sources.iter().map(|(path, source)| (path.as_str(), source.as_str())).collect();
                let (mut compiled, source_files) =
                    compile_fixtures_with_js(t, Some(version), &sources);
                let mut tp = compiled.type_parser();
                let result = compare_effect_context(t, &mut tp, &source_files);
                t.log(sprintf!(
                    "%d files, %d nodes, %d with an entry, %d can yield, %d in thunks, %d generator functions",
                    source_files.len() as i64,
                    result.nodes,
                    result.stored,
                    result.can_yield,
                    result.in_thunk,
                    result.generators.len() as i64
                ));
                if result.can_yield == 0 || result.in_thunk == 0 {
                    t.fatal("fixtures do not exercise the analysis");
                }
                if result.stored * 10 > result.nodes {
                    t.error(sprintf!(
                        "%d of %d nodes have an entry, want at most a tenth",
                        result.stored,
                        result.nodes
                    ));
                }
            });
        }
    });
}
