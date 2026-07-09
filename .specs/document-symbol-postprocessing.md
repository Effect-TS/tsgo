# Document Symbol Postprocessing

## Goal

Document the intended insertion point for adding additional document symbols on top of TypeScript-Go's native `textDocument/documentSymbol` output, while preserving the existing TypeScript-Go symbol collection behavior.

## Request Path

The `textDocument/documentSymbol` request flows through these stages:

1. The LSP server registers the request in `typescript-go/internal/lsp/server.go`.
2. `Server.handleDocumentSymbol` forwards the request to `LanguageService.ProvideDocumentSymbols`.
3. `LanguageService.ProvideDocumentSymbols` builds hierarchical document symbols via `getDocumentSymbolsForChildren`.
4. When a client does not support hierarchical symbols, the flat `SymbolInformation` response is derived from the same hierarchical result.

## Desired Outcome

Additional symbols should be layered on top of the upstream TypeScript-Go document symbol output rather than replacing the upstream symbol collection logic.

The extension point should satisfy these requirements:

- Native TypeScript-Go symbol discovery continues to run first.
- Expando merging remains intact before any Effect-specific symbol additions are considered.
- Additional symbol processing happens once, in a single place, before the final symbol tree is returned.
- The same augmented symbol result is used for both hierarchical `DocumentSymbol` responses and flattened `SymbolInformation` responses.

The first implementation slice is intentionally a stub that proves out the seam:

- expose a postprocessing callback registration point from `typescript-go/internal/ls/symbols.go`,
- register that callback from the Effect-side `etslshooks` package,
- prepend a synthetic top-level `effect` symbol,
- attach two synthetic children named `layer one` and `layer two`, and
- leave the native TypeScript-Go symbols in their existing order after that synthetic root.

This fixed structure is only a placeholder for later Effect-aware symbol extraction. Its value is validating the patch seam, ordering, flattening behavior, and baseline coverage before wiring real Effect facts into the symbol tree.

## Preferred Patch Seam

The preferred patch seam is in `typescript-go/internal/ls/symbols.go`, at the end of `getDocumentSymbolsForChildren`, immediately after the existing `mergeExpandos(symbols)` behavior.

At this point, the symbol tree has already been collected and normalized by upstream logic, which makes it the most stable place to append or transform additional symbols without interfering with the main AST walk.

## Constraints

Any postprocessing added at this seam must:

- Keep the upstream TypeScript-Go behavior as the base result.
- Remain deterministic and preserve stable ordering for tests.
- Be cheap enough to run on every document symbol request.
- Avoid widening the patch more than necessary in the `typescript-go` submodule.
- Be maintained through the local patch queue in `_patches/`, not as an untracked direct edit in `typescript-go/`.
- Keep Effect-specific symbol construction out of the `typescript-go` patch itself; the patch should only expose and invoke the callback seam, mirroring the existing hover/inlay/completion hook pattern.

For the initial stub, synthetic symbols should behave like normal document symbols:

- use a stable synthetic `SymbolKind` suitable for grouping, currently `Namespace`,
- appear at the beginning of the returned list,
- preserve child order exactly as produced by the postprocessor, and
- use valid ranges so both baseline generation and client flattening continue to work.

## Validation Expectations

A change that adds document-symbol postprocessing is complete only when:

- hierarchical document symbols still work,
- flattened symbol information still works,
- existing TypeScript-Go document-symbol baselines remain stable unless intentionally changed, and
- new or updated baselines cover the additional symbol behavior.
