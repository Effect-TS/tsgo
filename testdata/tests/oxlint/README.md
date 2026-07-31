# Oxlint bridge prototype

**PROTOTYPE: throwaway integration code, not a production package.**

This prototype answers whether synchronous Oxlint JavaScript visitors can share one persistent patched TypeScript-Go process, run the Effect semantic diagnostic pass once per file, and fan the result back out through one Oxlint rule per Effect rule.

The API boundary is split by consumer: `etsgoapi` provides in-process APIs for Go integrations, while `etsjsapi` owns the cross-process protocol used by JavaScript integrations. The code under `_packages/tsgo/src/experimental/oxlint` is an Oxlint-specific adapter over `etsjsapi`.

Run it from the repository root:

```sh
pnpm --filter effect-tsgo-oxlint-prototype test
```

The command builds the patched `tsgo` executable, obtains the pinned Oxlint CLI with `pnpm dlx`, and lints the local fixtures. Five `effect/floatingEffect` diagnostics and one warning with an Effect suggestion are expected. It then runs `--fix-suggestions` against a temporary copy and verifies that Oxlint applies the returned `yield*` edit.

## Effect options

Rules read shared Effect options from Oxlint settings when the `effect-tsgo` key exists:

```json
{
  "settings": {
    "effect-tsgo": {
      "pipeableMinArgCount": 3
    }
  }
}
```

When the key is absent, the server falls back to the TypeScript project's `@effect/language-service` options. The Oxlint adapter sends its enabled wrappers through `onlyRules`; when that field is present, the server replaces every registered Effect rule severity with `off` and sets only the selected rules to `error`. When `onlyRules` is omitted or `null`, the server instead runs rules as configured by `overrideEffectOptions` or the project tsconfig. This makes Oxlint the sole authority for its final reported severity while allowing other API consumers to use project configuration directly.

## Quick fixes

Protocol v3 exposes `runEffectDiagnostics` and can return non-disable Effect code actions with each diagnostic. `targetFilePath` selects the file, while optional `overrideSourceText` layers unsaved text over the project snapshot. The Oxlint adapter exposes every action as a suggestion, preserving all edits in the action as one group. Plain `--fix` does not apply these actions; users must opt in with `--fix-suggestions`. Automatic fixes are intentionally deferred until Effect actions carry explicit safety and preference metadata.

## Protocol types

The Go declarations in `etsjsapi/protocol.go` are the protocol source of truth. Generic method descriptors associate each method with its parameter and result types. Run `go generate ./etsjsapi` to update `_packages/tsgo/src/experimental/api/protocol.generated.ts`; `pnpm setup-repo` also runs this generator. The generated TypeScript `MethodMap` preserves the method-to-params/result relationship used by the synchronous client.

## Transport decision

The bridge copies and adapts TypeScript-Go's `SyncRpcChannel` transport under `_packages/tsgo/src/experimental/api/sync-channel.ts`. The copied code retains its process cleanup, blocking pipe descriptors, read-ahead buffering, large-read path, retry behavior, and Windows named-pipe connection. The adapter replaces MessagePack with an Effect-owned, versioned Content-Length JSON protocol:

- starts one long-lived `tsgo --effect-js-api` child with normal `spawn`;
- synchronously writes and reads the child pipe file descriptors with `fs.writeSync` and `fs.readSync`;
- uses stdio on POSIX and a uniquely named `effect-tsgo-js-api-sync-*` Windows pipe;
- implements Content-Length JSON framing instead of TypeScript-Go's general API protocol; and
- kills the child during explicit or process-exit cleanup.

`spawnSync` is not suitable for a persistent request loop because it returns only after the child exits. The copied channel uses normal `spawn` and blocking pipe I/O instead. The Go server's `--pipe` option delegates to TypeScript-Go's shimmed `PipeTransport`, which provides Windows named pipes and Unix domain sockets. The Effect adapter intentionally omits filesystem callbacks, binary AST responses, and the general TypeScript-Go API object model.

The copied transport is derived from Microsoft TypeScript-Go and remains subject to its Apache-2.0 license; attribution is retained at the top of `_packages/tsgo/src/experimental/api/sync-channel.ts`.
