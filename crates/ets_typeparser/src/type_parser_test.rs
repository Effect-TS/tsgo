// Ported from effect-tsgo's internal/typeparser/type_parser_test.go (see helpers_test.rs).
//
// Rust: Go's `Cached`/`CachedNode` take any link store; the Rust ones select a store of the parser's links
// (`EffectLinks`), so these tests run over a parser of an empty program and use a links store of the same shape
// as Go's local store (noted on each test).

use std::cell::Cell;

use ets_go::sprintf;
use ets_shim::ast::{Node, NodeFactoryHooks, Symbol, SymbolFlags};
use ets_shim::core::P;
use ets_shim::core::arena::Region;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/type_parser_test.go:11 TestCachedCachesNegativeValues 6d74e69dc80bbf1b
// Rust: the nil case uses a node-keyed store of optional values (Go: `core.LinkStore[string, *int]`), keyed by the
// source file node; the false case uses the string-keyed boolean store (Go: `core.LinkStore[string, bool]`).
#[test]
fn test_cached_caches_negative_values() {
    run(|t| {
        t.parallel();

        t.run("nil", |t| {
            t.parallel();

            let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, "");
            let mut tp = compiled.type_parser();
            let key = sf.as_node();
            let mut calls = 0;
            let mut compute = |_: &mut TypeParser<'_>| {
                calls += 1;
                None
            };

            if cached(&mut tp, |l| &mut l.object_literal_contextual_type, key, &mut compute)
                .is_some()
            {
                t.fatal("expected nil result");
            }
            if cached(&mut tp, |l| &mut l.object_literal_contextual_type, key, &mut compute)
                .is_some()
            {
                t.fatal("expected cached nil result");
            }
            if calls != 1 {
                t.fatal(sprintf!(
                    "expected nil result to be computed once, got %d calls",
                    calls as i64
                ));
            }
        });

        t.run("false", |t| {
            t.parallel();

            let (mut compiled, _) = compile_and_get_checker_and_source_file_internal(t, "");
            let mut tp = compiled.type_parser();
            let mut calls = 0;
            let mut compute = |_: &mut TypeParser<'_>| {
                calls += 1;
                false
            };

            if cached(
                &mut tp,
                |l| &mut l.global_interface_has_pipe,
                "key".to_string(),
                &mut compute,
            ) {
                t.fatal("expected false result");
            }
            if cached(
                &mut tp,
                |l| &mut l.global_interface_has_pipe,
                "key".to_string(),
                &mut compute,
            ) {
                t.fatal("expected cached false result");
            }
            if calls != 1 {
                t.fatal(sprintf!(
                    "expected false result to be computed once, got %d calls",
                    calls as i64
                ));
            }
        });
    });
}

// go test: internal/typeparser/type_parser_test.go:57 withGlobalNodeId 2bf2f52b1d1f8aba
fn with_global_node_id(node: P<Node>) -> P<Node> {
    ets_shim::ast::get_node_id(node);
    node
}

// go test: internal/typeparser/type_parser_test.go:62 withGlobalSymbolId f83c1fd452709687
fn with_global_symbol_id(symbol: P<Symbol>) -> P<Symbol> {
    ets_shim::ast::get_symbol_id(symbol);
    symbol
}

// go test: internal/typeparser/type_parser_test.go:67 TestCachedNodeCachesPerNode 9716d4d44279cca4
// Rust: the store is a node-keyed store of optional nodes (Go: `checker.NodeLinkStore[*string]`); the values are
// identifiers whose text is Go's string.
#[test]
fn test_cached_node_caches_per_node() {
    run(|t| {
        t.parallel();

        let (mut compiled, _) = compile_and_get_checker_and_source_file_internal(t, "");
        let mut tp = compiled.type_parser();

        let factory = ets_shim::ast::new_node_factory(NodeFactoryHooks::default());
        let first = factory.new_identifier("first");
        let second = factory.new_identifier("second");
        let numbered = with_global_node_id(factory.new_identifier("numbered"));

        let calls = Cell::new(0);
        let compute = |value| {
            let calls = &calls;
            move |_: &mut TypeParser<'_>| {
                calls.set(calls.get() + 1);
                value
            }
        };
        let second_value = factory.new_identifier("second");
        let numbered_value = factory.new_identifier("numbered");
        let text = |value: Option<P<Node>>| value.map(|value| value.text().to_string());

        if tp.links().effect_yield_generator_function.contains_key(&first) {
            t.fatal("expected an empty store");
        }
        if cached_node(&mut tp, |l| &mut l.effect_yield_generator_function, first, compute(None))
            .is_some()
        {
            t.fatal("expected nil result for first");
        }
        let got = text(cached_node(
            &mut tp,
            |l| &mut l.effect_yield_generator_function,
            second,
            compute(Some(second_value)),
        ));
        if got.as_deref() != Some("second") {
            t.fatal(sprintf!("expected %q for second, got %v", "second", got));
        }
        let got = text(cached_node(
            &mut tp,
            |l| &mut l.effect_yield_generator_function,
            numbered,
            compute(Some(numbered_value)),
        ));
        if got.as_deref() != Some("numbered") {
            t.fatal(sprintf!("expected %q for numbered, got %v", "numbered", got));
        }
        if cached_node(
            &mut tp,
            |l| &mut l.effect_yield_generator_function,
            first,
            compute(Some(second_value)),
        )
        .is_some()
        {
            t.fatal("expected cached nil result for first");
        }
        let got = text(cached_node(
            &mut tp,
            |l| &mut l.effect_yield_generator_function,
            numbered,
            compute(None),
        ));
        if got.as_deref() != Some("numbered") {
            t.fatal(sprintf!("expected cached %q for numbered, got %v", "numbered", got));
        }
        {
            let links = tp.links();
            let store = &links.effect_yield_generator_function;
            if !store.contains_key(&first)
                || !store.contains_key(&second)
                || !store.contains_key(&numbered)
            {
                t.fatal("expected every computed node to be present");
            }
        }
        if calls.get() != 3 {
            t.fatal(sprintf!(
                "expected one computation per node, got %d calls",
                calls.get() as i64
            ));
        }
    });
}

// go test: internal/typeparser/type_parser_test.go:111 TestSymbolLinkStoreKeepsOneSlotPerSymbol e23d6769422da4d5
// Rust: Go's `checker.SymbolLinkStore[V]` is a symbol-keyed map (`FxHashMap<P<Symbol>, V>`, as in `EffectLinks`):
// `TryGet` is `get`, `Get` is `entry(..).or_default()`.
#[test]
fn test_symbol_link_store_keeps_one_slot_per_symbol() {
    run(|t| {
        t.parallel();

        let region = Region::new_scratch(1 << 16);
        let _scope = region.enter();
        let (first, second) =
            (Symbol::new(SymbolFlags::None, ""), Symbol::new(SymbolFlags::None, ""));
        let numbered = with_global_symbol_id(Symbol::new(SymbolFlags::None, ""));

        let mut store: FxHashMap<P<Symbol>, String> = FxHashMap::default();
        if store.contains_key(&first) {
            t.fatal("expected an empty store");
        }
        *store.entry(first).or_default() = "first".to_string();
        *store.entry(second).or_default() = "second".to_string();
        *store.entry(numbered).or_default() = "numbered".to_string();

        for (symbol, want) in [(first, "first"), (second, "second"), (numbered, "numbered")] {
            let got = store.get(&symbol).cloned();
            if got.as_deref() != Some(want) {
                t.fatal(sprintf!("expected %q from TryGet, got %v", want, got));
            }
            let got = store.entry(symbol).or_default().clone();
            if got != want {
                t.fatal(sprintf!("expected %q from Get, got %q", want, got));
            }
        }
    });
}
