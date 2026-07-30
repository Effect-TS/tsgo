# Oxlint bridge prototype

**PROTOTYPE: throwaway integration code, not a production package.**

This prototype answers whether synchronous Oxlint JavaScript visitors can share one persistent patched TypeScript-Go process, run the Effect semantic diagnostic pass once per file, and fan the result back out through one Oxlint rule per Effect rule.

Run it from the repository root:

```sh
pnpm prototype:oxlint-bridge
```

The command builds the patched `tsgo` executable, obtains the pinned Oxlint CLI with `pnpm dlx`, and lints the two local floating-Effect fixtures. Five `effect/floatingEffect` diagnostics are expected. Oxlint exits with status 1 when it finds them; the runner treats that as a successful prototype result.

Set `EFFECT_TSGO_BRIDGE_TRACE=0` to hide broker state. The default trace shows one persistent client and one `lint` request per file, including cold and warm request timings.

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

When the key is absent, the server falls back to the TypeScript project's `@effect/language-service` options. In either case, the server replaces diagnostic severities before running rules: every registered Effect rule is `off`, and every wrapper enabled by Oxlint is `error`. The requested rule list also limits execution. This makes Oxlint the sole authority for the final reported severity while preserving non-severity Effect options and path-scoped overrides.

## Transport decision

The bridge copies and adapts TypeScript-Go's `SyncRpcChannel` transport under `_packages/tsgo/src/experimental/oxlint/sync-channel.ts`. The copied code retains its process cleanup, blocking pipe descriptors, read-ahead buffering, large-read path, retry behavior, and Windows named-pipe connection. The adapter replaces MessagePack with an Effect-owned, versioned Content-Length JSON protocol:

- starts one long-lived `tsgo --effect-oxlint` child with normal `spawn`;
- synchronously writes and reads the child pipe file descriptors with `fs.writeSync` and `fs.readSync`;
- uses stdio on POSIX and a uniquely named `effect-tsgo-oxlint-sync-*` Windows pipe;
- implements Content-Length JSON framing instead of TypeScript-Go's general API protocol; and
- kills the child during explicit or process-exit cleanup.

`spawnSync` is not suitable for a persistent request loop because it returns only after the child exits. The copied channel uses normal `spawn` and blocking pipe I/O instead. The Go server's `--pipe` option delegates to TypeScript-Go's shimmed `PipeTransport`, which provides Windows named pipes and Unix domain sockets. The Effect adapter intentionally omits filesystem callbacks, binary AST responses, and the general TypeScript-Go API object model.

The copied transport is derived from Microsoft TypeScript-Go and remains subject to its Apache-2.0 license; attribution is retained at the top of `_packages/tsgo/src/experimental/oxlint/sync-channel.ts`.
