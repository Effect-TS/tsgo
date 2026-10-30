# Known divergences between the Rust port and the Go build

Every observable difference between the Rust port and the Go code at the same commit that we accept. Anything not
listed here is a bug (`PORTING.md`, "The fidelity rule"). An entry needs evidence; "looks like a TypeScript
difference" is not enough.

Kinds:

- `typescript`: tsrs's TypeScript snapshot and the Go build's TypeScript check or print something differently.
  Evidence: the Go build at tsrs's TypeScript version shows the same output as Rust (or the upstream TypeScript
  change responsible, with its commit).
- `tsrs`: a tsrs bug, i.e. tsrs differs from tsgo at tsrs's own TypeScript pin. Evidence: a reduced input and the
  tsrs issue (or the reason it is not filed).
- `library`: a Go library or runtime behavior the Rust side cannot reproduce. Evidence: the specific difference and
  why it cannot be matched.
- `scope`: a feature left out on purpose (`PORTING.md`, "Scope").
- `types`: a run-time check Go needs that Rust's types turn into a compile error. Evidence: the Go code that checks
  at run time, and a `compile_fail` test.

Format: one row per difference. "Affected" lists the baselines (`testdata/baselines/reference/...`) and ported
tests whose Rust expectation differs because of it, or `none`.

| Area | Kind | Difference | Evidence | Affected |
| ---- | ---- | ---------- | -------- | -------- |
| `--effect-cli-diagnostics` request decoding | library | A request that is not valid JSON is reported as `invalid diagnostics request: <error>` with serde_json's error text (`expected value at line 1 column 1`) where Go prints encoding/json's (`invalid character 'x' looking for beginning of value`). Type errors and the top-level kind errors use Go's texts | `ets_etsdiagnostics::decode_request` / `decode_any` decode with serde_json; reproducing encoding/json's scanner messages would mean porting its scanner | none |
| `layergraph.EncodeMermaidURL` | library | The `pako:` fragment's compressed bytes differ from Go's: Go's `compress/zlib` at `BestCompression` and `flate2`'s zlib encoder at `Compression::best()` pick different (equally valid) deflate encodings. The fragment decodes to the same JSON payload | `ets_layergraph::encode_mermaid_url`; matching byte for byte would mean porting Go's `compress/flate` compressor. `mermaidurl_test` checks the decoded roundtrip, as the Go tests do. No reference baseline embeds a Mermaid URL: `.layers.txt` holds the flowcharts and the quick-info summary (`FormatQuickInfo`, no links) and `.flows.*.mermaid` the flowchart text, so they match byte for byte | `internal/effecttest/hover_test.go` `TestEffectHoverLayerQuickInfo`, `TestEffectHoverLayerMermaidProvider` (their Mermaid links are compared decoded: `ets_effecttest` `tests/effecttest/hover_test.rs`, `verify_quick_info_at_decoded`) |
| `layergraph` 50-byte label truncation (`mermaidSafe`, `FormatQuickInfo`) | library | When the cut splits a multi-byte UTF-8 character, Go keeps the leading bytes (invalid UTF-8) and Rust writes U+FFFD in their place | Go's `s[:50]` slices bytes; a Rust `String` cannot hold invalid UTF-8 (`ets_layergraph::format::truncate_bytes`) | none |
| `layerMergeAllWithDependencies` message type order | library | When one `Layer.mergeAll` argument provides several types that other arguments require, Go lists their names in the random iteration order of `actuallyProvidedMap` (a Go map); Rust lists them in insertion order (the provider's `ROut` union order, by argument), the deterministic order that follows the source | `internal/rules/layer_merge_all_with_dependencies.go` `analyzeLayerMergeAllCall` ranges over a map; `ets_rules::analyze_layer_merge_all_call` keeps an insertion-ordered vector. The current baselines have one provided type per provider, so they match either way | none |
| `etslshooks` `layerSymbolName` 80-byte truncation | library | When the cut at byte 77 splits a multi-byte UTF-8 character, Go keeps the leading bytes (invalid UTF-8) and Rust writes U+FFFD in their place; a class expression without a name (Go's `scanner.GetTextOfNode(nil)`, a panic) reads as `""` | Go's `text[:77]` slices bytes; a Rust `String` cannot hold invalid UTF-8 (`ets_etslshooks` `document_symbols.rs`, `layer_symbol_name`), as in the layergraph truncation row | none |
| `schemagen` structural generator map order | library | `StructuralSchemaGen` iterates Go maps (`hoistedSchemas` when reusing a hoisted schema, `nameToType` when resolving a hoist name, the requested type map in `Process`) in Go's random order; Rust iterates them in insertion order. Only observable when several entries match (two hoisted schemas whose types are mutually assignable to the processed type), where Go picks one at random | `internal/schemagen/structural.go` ranges over the maps; `ets_schemagen::StructuralSchemaGen` keeps `OrderedMap`s. Every caller requests one type, and the `structuralTypeToSchema*` baselines match | none |
