# Oxlint JavaScript Rules to Persistent TypeScript-Go: Feasibility Assessment

**Date:** 2026-07-29
**Status:** Research and API proof complete; Oxlint plugin implementation not started
**Question:** Can an Oxlint JavaScript custom rule synchronously query type information from a persistent, patched `typescript-go` process, including in CLI and editor workflows?

## Executive conclusion

Yes. For this repository's existing Effect diagnostics, the idea is more directly feasible than a general typed-JavaScript-rule bridge.

The shortest viable bridge is a module-level instance of the official `@typescript/native-preview/unstable/sync` API. It starts one persistent `tsgo --api` child and performs blocking MessagePack RPC over the child's pipes. That execution model fits Oxlint because rule creation, hooks, and AST visitors are synchronous, and Oxlint already serializes JavaScript lint callbacks onto Node's main JavaScript thread while the calling Rust worker waits. [TSGO-SYNC-CLIENT] [TSGO-SYNC-CHANNEL] [OXC-CALLBACK]

Crucially, the patched binary already needs no new checker-query API for a diagnostic-only prototype. `Program.getSemanticDiagnostics(file)` reaches the checker callback registered by this repository, that callback invokes `rulerunner.Run` once for the source file, and the returned diagnostics contain UTF-16 ranges and stable `377xxx` codes. Those codes already map one-to-one back to Effect rule names in generated metadata. One synchronous request can therefore run the existing Go rules once; one-to-one Oxlint JavaScript rule wrappers only filter and report their portion of that shared result. [TSGO-DIAGNOSTICS-API] [EFFECT-HOOK] [EFFECT-RUNNER] [EFFECT-METADATA]

A local proof on the inspected revision used the current API client against a freshly built patched binary. A disk-backed `floatingEffect.ts` returned four code `377001` diagnostics, a temporary snapshot containing unsaved replacement text returned none, and the unchanged base snapshot still returned the original four afterward. With all 94 Effect rules enabled, the warm per-file semantic diagnostic request took 3.43 ms in this fixture; this is only a feasibility measurement, not a representative project benchmark.

This is not a transparent way to run arbitrary type-aware ESLint plugins. Oxlint explicitly documents type-aware JavaScript rules as unsupported, exposes an empty `parserServices` object, supplies no ESTree-to-TypeScript-node maps, and does not await promises returned by visitors. Positional checker queries remain a possible fallback for new bridge-aware rules, but they are unnecessary for reusing the diagnostics already implemented here. [OXC-DOC-JS] [OXC-PARSER-SERVICES] [OXC-VISITORS]

The existing Oxlint-to-tsgolint integration is not a reusable persistent bridge. Oxlint starts `tsgolint headless` for each type-aware run, writes one JSON payload, closes stdin, consumes framed diagnostics, and kills the child. Tsgolint constructs programs and checkers inside that one invocation, and its rules are Go functions registered in the binary rather than dynamically loaded JavaScript rules. [OXC-TSGOLINT] [TGL-HEADLESS] [TGL-WORKLOAD] [TGL-RULES]

The recommended proof of concept is therefore:

1. Generate one Oxlint wrapper rule per existing Effect rule from `metadata.json`.
2. Lazily create one synchronous TypeScript-Go API client per Oxlint workspace/CWD.
3. Let every enabled wrapper register itself in its `Program` visitor. On a successful traversal, all combined `Program` enter callbacks run before any `Program:exit` callback, so this discovers the complete enabled-wrapper set without relying on `before`.
4. The first `Program:exit` callback asks a shared broker for Effect semantic diagnostics. The broker updates one persistent base snapshot, layers `context.sourceCode.text` through `runWithTemporaryFileUpdate` when needed, calls `getSemanticDiagnostics(file)` once, filters codes `377000..377999`, and caches diagnostics by Effect rule name for the rest of that file invocation.
5. Each wrapper's `Program:exit` reports only its diagnostics; the last exit clears the per-file frame. An idempotent `after` hook may provide error-path cleanup, but correctness must not depend on it running for every file.
6. Keep and explicitly dispose only the newest base snapshot. Measure update, temporary-overlay, semantic-check, and serialized-callback wall time separately.

This design is suitable for a controlled CLI prototype and for editor diagnostics that need the current file's unsaved contents. Correct type information involving *other* unsaved editor files requires either attachment to TypeScript-Go's existing LSP project session or a new complete multi-file overlay channel. The current official synchronous client cannot attach to a socket, while the official socket client is asynchronous, so full editor-state sharing is the largest unresolved integration seam. [TSGO-SYNC-CLIENT] [TSGO-ASYNC-CLIENT] [TSGO-LSP-API]

## Verdict by use case

| Use case | Verdict | Reason |
| --- | --- | --- |
| Existing Effect diagnostics, CLI, files on disk | Feasible now as a prototype | One semantic-diagnostics request already runs this repository's existing checker hook and returns stable codes and UTF-16 ranges. [TSGO-DIAGNOSTICS-API] [EFFECT-HOOK] |
| Existing Effect diagnostics, unsaved current editor file | Feasible and locally proven | A temporary snapshot accepts the current file's full text and is released after the synchronous callback. [TSGO-API-CLIENT] |
| Standalone native Oxlint binary | Not a target for this plugin design | Oxlint's external JavaScript linter is supplied by the Node/N-API entry point and currently only enabled on 64-bit little-endian targets; users must run the npm/Node distribution. [OXC-NODE-RUNTIME] |
| Bespoke JS rule, multiple unsaved imported editor files | Not correct with a standalone child today | Oxlint gives a rule the current file, not the editor's complete open-document set; a one-file temporary snapshot cannot carry all dependency overlays. [OXC-LSP-SOURCE] [TSGO-PROJECT-API] |
| Attach a JS rule to an existing TypeScript-Go LSP session | Architecturally available, but blocked for synchronous visitors | The LSP can create a shared API session on a socket, but its API connection and official socket client are asynchronous. [TSGO-LSP-API] [TSGO-ASYNC-CLIENT] |
| Independent Oxlint enablement/configuration | Needs a small Effect-specific API extension for full parity | The generic semantic method uses the Effect configuration parsed from `tsconfig`; JavaScript-side filtering cannot resurrect a rule the Go runner skipped. [EFFECT-RUNNER] |
| Oxlint autofix parity | Not available through the generic API | The generic diagnostic response contains no fixes, and Effect fixes are currently exposed through the language-service code-fix provider. [TSGO-DIAGNOSTIC-SHAPE] [EFFECT-FIXES] |
| Drop-in execution of existing type-aware ESLint plugins | Not feasible without a compatibility layer | `parserServices` is empty and there are no ESTree/TypeScript node maps. [OXC-PARSER-SERVICES] |
| Reuse tsgolint as the persistent custom-rule host | Poor fit | Its protocol is one-shot and its rule registry consists of compiled Go rules. [OXC-TSGOLINT] [TGL-RULES] |
| Add missing TypeScript queries through this repository's patches | Feasible | TypeScript-Go already has an API boundary; additions should be narrow API methods carried in `_patches`, not direct persistent edits to the submodule. [REPO-POLICY] |

## Scope and source baseline

This assessment uses first-party source and documentation only.

| Project | Revision inspected | Local source |
| --- | --- | --- |
| Oxc/Oxlint | [`a065946a8ce95eb3374e08242cd9086ab050314b`](https://github.com/oxc-project/oxc/tree/a065946a8ce95eb3374e08242cd9086ab050314b) | `.repos/oxlint` |
| tsgolint | [`16a224c6cc96e4111cc6edfeded8e3028c2b59ce`](https://github.com/oxc-project/tsgolint/tree/16a224c6cc96e4111cc6edfeded8e3028c2b59ce) | `.repos/tsgolint` |
| TypeScript-Go | [`70c2f5e51856a908b05ac98b5e954b4c685520dd`](https://github.com/microsoft/typescript-go/tree/70c2f5e51856a908b05ac98b5e954b4c685520dd) plus this repository's existing patch set | `typescript-go` |
| Effect TypeScript-Go | [`c9b44998eaf5c8df0f18a1bdb8dc95376d259de2`](https://github.com/Effect-TS/tsgo/tree/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2) | repository root |

The TypeScript-Go API files cited below are unmodified at the inspected checkout. The local patch set changes other TypeScript-Go files; the only cited file that is locally modified is `internal/lsp/server.go`, where the local change adds an unrelated code-action kind. Source links are pinned to the upstream base revision.

### Local API proof

The feasibility proof built the patched `typescript-go/cmd/tsgo` at the repository revision above and loaded the matching synchronous client directly from the pinned TypeScript-Go source tree. A virtual `tsconfig.json` enabled the Effect plugin while the program resolved the physical `testdata/tests/effect-v4` fixture and dependencies.

The proof made these assertions:

- Base snapshot: `floatingEffect.ts` produced four diagnostics with code `377001` and expected UTF-16 ranges.
- Temporary snapshot: replacing the file text with an assigned Effect value produced zero Effect diagnostics.
- Snapshot isolation: querying the base snapshot again still produced the original four diagnostics.
- Full registry: enabling all 94 metadata rules still issued one semantic request; after timing counters were reset, that request measured 3.43 ms server round trip on this machine and fixture.

The proof used the real `API.updateSnapshot`, `runWithTemporaryFileUpdate`, `getDefaultProjectForFile`, and `Program.getSemanticDiagnostics` paths cited below. It did not simulate the Oxlint wrapper lifecycle, fixes, multi-file unsaved state, or representative project performance.

## Current architecture

### Oxlint's JavaScript execution contract

An Oxlint plugin consists only of metadata and a rule map. A rule has either `create(context)` or Oxlint's alternative `createOnce(context)` method; no plugin shutdown or disposal callback is part of the plugin shape. [OXC-PLUGIN-SHAPE]

`create` executes once per enabled rule per file. `createOnce` executes during plugin registration, and its visitor plus optional `before` and `after` hooks are retained. Oxlint creates one context per rule during plugin loading and reuses it across files. [OXC-PLUGIN-LOAD] [OXC-CONTEXT]

For each file, Oxlint switches the global workspace, installs file-specific values into reused contexts, executes `create` or the retained `before` hook, combines all enabled visitors, walks the AST once, and then executes retained `after` hooks. [OXC-LINT-FILE]

The execution contract is synchronous:

- `BeforeHook` returns `boolean | void` and `AfterHook` returns `void`.
- Generated walkers directly call `visit(node)`, `enter(node)`, and `exit(node)` and discard return values.
- Plugin module loading itself is asynchronous through `await import(...)`, but that does not make rule execution asynchronous.

Consequently, an async API call started inside a visitor cannot be awaited by Oxlint, and reporting after the visitor returns is unsafe because the reused file context is reset after each file. [OXC-VISITORS] [OXC-PLUGIN-IMPORT] [OXC-LINT-FILE]

Oxlint's native file pipeline may process files in parallel, but JavaScript lint callbacks are N-API `ThreadsafeFunction` calls executed on the main JavaScript thread. Each Rust caller blocks on a channel until that callback completes, and a callback may wait behind an earlier file's callback. A blocking TypeScript RPC inside a JS rule therefore preserves correctness but lengthens an already serialized section and stalls the corresponding Rust worker. [OXC-PARALLEL] [OXC-CALLBACK]

Oxlint converts AST, token, and comment spans from Rust's UTF-8 byte offsets to UTF-16 before exposing them to JavaScript. JavaScript diagnostics and fixes are converted back to UTF-8 by Rust. This makes an Oxlint ESTree node's `start`, `end`, or `range` directly usable as TypeScript-Go API positions, subject to the normal choice of which point within a node should be queried. [OXC-AST-SPANS] [OXC-JS-DIAGNOSTICS]

#### Exact `before`-hook guarantee

Oxlint's official alternative-API documentation explicitly says `before` is not guaranteed to run on every file. The reason given is a planned Rust-side optimization: once Oxlint knows which AST node types a `createOnce` visitor is interested in, it may omit the JavaScript call for a file containing none of those types. The documented way to require per-file visitor work is a `Program` visitor. [OXC-DOC-CREATE-ONCE]

That node-interest optimization is not implemented for external JavaScript rules at the inspected revision. A successfully reached JavaScript invocation currently loops over every external rule ID enabled for the source section and calls each non-null `before` hook before compiling the combined visitor, regardless of whether the AST contains a matching node. The analogous `contains_any` optimization currently applies only to native Rust rules. [OXC-BEFORE-RUN] [OXC-NATIVE-NODE-SKIP]

There is already one deliberate exception. During plugin registration, Oxlint removes `before` and `after` from the returned visitor and checks `Object.keys(visitor)`. If no own enumerable AST visitor remains, it replaces the user's `before` with an internal hook that returns `false` and drops `after`; the user's hooks are never called. Merged PR [#14401](https://github.com/oxc-project/oxc/pull/14401) introduced this behavior specifically to prevent hooks-only rules from depending on an every-file guarantee that the planned optimization would later break. [OXC-BEFORE-REGISTER] [OXC-PR-BEFORE-EMPTY]

For a nonempty visitor, the user's `before` is still omitted when no successful invocation reaches that rule:

- The rule is `off` for the file after base, nested, and override configuration is resolved, so its rule ID is not sent to JavaScript. [OXC-RULE-ENABLEMENT]
- The path is unsupported, minified, ignored, or otherwise excluded; `--type-check-only` bypasses regular lint rules; reading fails; or parsing/semantic construction fails before a lint context is created. A partial-loader file may invoke hooks once per successfully parsed script section rather than exactly once per containing file. [OXC-FILE-SELECTION] [OXC-LINT-SEQUENCE] [OXC-PARSE-GATE]
- Plugin loading or `createOnce` registration fails, JavaScript plugins are unavailable in the selected Oxlint host/platform, or an earlier rule throws during per-file setup, `create`, `before`, or visitor compilation. Rule order is indeterminate, so such an exception can prevent a later enabled rule's hook from being reached. Oxlint's hook-error fixture verifies this behavior. [OXC-BEFORE-RUN] [OXC-BEFORE-ERROR-TEST] [OXC-NODE-RUNTIME]

Returning `false` from a rule's own `before` is not an omitted call: that hook ran, and Oxlint then skips only that rule's visitor and `after`. Inline `oxlint-disable` directives also do not suppress the hook; Oxlint executes the JavaScript rule and filters its returned diagnostics afterward. [OXC-BEFORE-RUN] [OXC-JS-DIAGNOSTICS]

For this bridge, every generated wrapper can declare both `Program` and `Program:exit`. `Program` registers the wrapper and current context; the walker invokes all combined `Program` enter callbacks before descending into the body, then invokes `Program:exit` after traversal. The first exit can therefore compute the shared result after all enabled wrappers have registered, and each remaining exit can reuse it. This follows Oxlint's own recommendation and remains compatible with the planned node-interest optimization because every parsed source section necessarily has a `Program` node. [OXC-DOC-CREATE-ONCE] [OXC-PROGRAM-WALK]

### Why normal typed ESLint plugins do not work

Oxlint's first-party documentation says JavaScript plugins are alpha and lists rules relying on TypeScript type awareness as unsupported. [OXC-DOC-JS]

At source level, `SourceCode.parserServices` is a frozen empty object. There is no TypeScript `Program`, no `esTreeNodeToTSNodeMap`, and no `tsNodeToESTreeNodeMap`. [OXC-PARSER-SERVICES]

This leaves two distinct targets:

- **Bridge-aware custom rules:** feasible. They can import a bridge API and query by file plus UTF-16 position.
- **Unmodified typed ESLint rules:** infeasible without an adapter that implements the parser-services contract and node identity maps those rules expect.

A positional bridge avoids materializing and mapping a complete second AST, but it is not semantically identical to a node map. TypeScript-Go's positional handlers choose `astnav.GetTouchingPropertyName` at the supplied UTF-16 offset. A rule must define stable position-selection conventions for identifiers, property names, calls, type nodes, and synthetic/zero-width constructs. [TSGO-POSITION-HANDLERS]

### Goal-specific shortcut: run the existing Effect diagnostic pass

This repository's checker patch calls a registered callback after TypeScript has checked a source file. `etscheckerhooks` registers that callback and invokes `rulerunner.Run(..., nil)`, where `nil` selects all Effect rules. Each returned diagnostic is added to the same checker diagnostic collection that backs normal semantic diagnostics. [EFFECT-CHECKER-PATCH] [EFFECT-HOOK] [EFFECT-RUNNER]

The existing TypeScript-Go API already exposes `Program.getSemanticDiagnostics(file)`. Its server handler resolves the requested source file, invokes `program.GetSemanticDiagnostics`, and serializes each diagnostic with UTF-16 `pos`/`end`, code, category, text, chains, and related information. It does not serialize code fixes. [TSGO-DIAGNOSTICS-API] [TSGO-DIAGNOSTIC-SHAPE]

This gives a prototype a direct fan-out path:

1. Make one semantic-diagnostics API call for the current file and snapshot.
2. Retain only codes in the reserved Effect range `377000..377999`.
3. Map code to rule name using the already generated metadata shipped by this repository.
4. Let each enabled Oxlint wrapper call `context.report` for only its mapped diagnostics.

The metadata already includes every rule's name, diagnostic codes, default severity, fixability, description, and supported Effect versions, so the JavaScript plugin definitions can be generated from the same source as the CLI/setup UI rather than maintained by hand. [EFFECT-METADATA]

No TypeScript AST needs to cross the process boundary for this design. Oxlint permits `context.report` with any object carrying a `range`, and both the API response and Oxlint's JavaScript ranges use UTF-16 offsets. A wrapper can report a diagnostic using `{ range: [diag.pos, diag.end] }` without finding the corresponding ESTree node. [OXC-REPORT] [OXC-AST-SPANS] [TSGO-DIAGNOSTIC-SHAPE]

#### Coordinating one request across one-to-one rules

Oxlint combines all enabled visitors into one traversal, and rule execution order is explicitly indeterminate. Coordination does not need that order: each generated `Program` visitor registers its rule and context in a broker frame; the first generated `Program:exit` computes and caches the full result after every `Program` entry has run; every exit reads its own bucket; and the last exit clears the frame. An `after` hook can clear an incomplete frame if traversal aborts, but it is an idempotent fallback rather than the registration barrier. [OXC-LINT-FILE] [OXC-PROGRAM-WALK]

All JavaScript file callbacks execute on Node's main thread, so that module-level broker and one synchronous client do not need cross-thread locking. Native Oxlint file workers remain parallel outside this serialized JavaScript section, but each worker that reaches a JavaScript callback waits for it. [OXC-CALLBACK] [OXC-PARALLEL]

For CLI there is normally one broker/client for the process. For a multi-root Oxlint LSP, the client must be keyed by workspace/CWD because the TypeScript-Go child fixes its CWD at startup. Oxlint currently has no reliable plugin-level disposal event, so process-exit cleanup remains necessary. [TSGO-SYNC-CLIENT] [OXC-WORKSPACE]

#### What the generic semantic endpoint does not solve

The generic endpoint obeys the Effect plugin options parsed from `tsconfig.json`. It cannot run a rule that `rulerunner.Run` skipped because the Effect diagnostics feature or that rule's severity is off. JavaScript-side filtering therefore gives correct one-to-one reporting only when TypeScript configuration has enabled the desired rules. [EFFECT-RUNNER]

That is acceptable for a proof of concept whose source of truth remains the existing Effect configuration. It is not ideal product behavior for independently configurable Oxlint rules. A production endpoint should accept the selected rule names and lint options from the broker, invoke the existing runner once, and return diagnostics grouped by rule. `rulerunner.Run` already accepts a `ruleNames` selection, so most of the missing work is API plumbing rather than rule refactoring. [EFFECT-RUNNER]

The generic response also lacks fixes. Effect quick fixes are currently computed by a language-service `CodeFixProvider` after a diagnostic, while tsgolint's headless protocol carries fixes and suggestions directly with each rule diagnostic. Diagnostic parity can ship before fix parity, but production Oxlint autofixes need an Effect-specific response that includes current-file text edits or a new general code-fix API. [EFFECT-FIXES] [TGL-HEADLESS]

### Existing Oxlint-to-tsgolint path

Oxlint's normal lint service runs before its type-aware linter in both CLI and source/LSP paths. The tsgolint result is appended after regular Oxlint messages; it does not provide type information during the JavaScript AST traversal. [OXC-LINT-SEQUENCE]

For every type-aware source run, Oxlint:

1. Reads every requested source through the active Oxlint filesystem.
2. Embeds those texts in `source_overrides`.
3. Spawns `tsgolint headless`.
4. Writes one serialized payload to stdin and closes stdin.
5. Reads length-prefixed diagnostic messages from stdout.
6. Kills the child after collection as cleanup.

There is no request loop and no retained process handle between lint runs. [OXC-TSGOLINT]

Tsgolint reads all of stdin once, deserializes V1 or V2 JSON, wraps the OS filesystem if source overrides are present, assigns files to tsconfig-backed or inferred programs, runs the linter, streams five-byte-header messages, and exits. [TGL-HEADLESS] [TGL-PAYLOAD]

Within one invocation, tsgolint does useful reuse and parallelism:

- It creates one program per discovered tsconfig and one inferred program for unmatched files.
- It builds a queue shared by checker workers for each program.
- Each worker reuses a rule context builder and listener arrays across files.
- All rule listeners themselves are synchronous Go functions over TypeScript AST nodes.

That state disappears when the headless process exits. [TGL-WORKLOAD] [TGL-LINTER]

Tsgolint cannot dynamically host a JavaScript rule. Its rule interface is a Go `Run` function returning Go node listeners, and the binary declares a static `allRules` slice. [TGL-RULE-API] [TGL-RULES]

Tsgolint reaches TypeScript-Go internals through generated shim modules and `go:linkname`; extra shim configuration can expose private methods. That mechanism is useful for Go-native rules but does not itself create a JavaScript RPC surface. [TGL-SHIMS]

The parts worth copying are its product-level data model, not its process lifecycle:

- Oxlint resolves enabled rules and options per file before invoking the typed backend.
- The payload carries exact source overrides read through Oxlint's active filesystem.
- Every returned diagnostic identifies its rule, file, range, message, fixes, and suggestions.
- TypeScript work is grouped by configured or inferred program and rules run against one checker-backed view per program.
- Oxlint owns final severity and disable-directive filtering.

The parts not worth copying are spawning per run, closing stdin as the end-of-request marker, reconstructing all programs, and streaming a one-shot response. A persistent TypeScript-Go snapshot replaces those pieces. [OXC-TSGOLINT] [TGL-HEADLESS] [TGL-WORKLOAD]

## TypeScript-Go's existing persistent API

### Process and transport

TypeScript-Go already exposes the required process lifetime. `tsgo --api` starts a server using stdio by default or a named pipe/Unix-domain socket with `--pipe`. Default mode uses MessagePack and synchronous request handling; `--async` selects JSON-RPC. [TSGO-CLI] [TSGO-SERVER]

The synchronous server reads and handles one request at a time inline. The asynchronous server handles each request or notification in a separate goroutine. [TSGO-CONN-SYNC] [TSGO-CONN-ASYNC]

The preview npm package exports first-party clients as `@typescript/native-preview/unstable/sync` and `@typescript/native-preview/unstable/async`. The package is itself private and version `0.0.0`, and the exports are explicitly under `unstable`, so a bridge must pin the TypeScript-Go binary and JavaScript client from the same source revision. [TSGO-PACKAGE] [TSGO-SYNC-CHANNEL]

The synchronous client is particularly relevant to Oxlint:

- It starts `tsgo --api` once in its constructor.
- It uses `fs.readSync` and `fs.writeSync` directly on child pipe descriptors.
- Each API method blocks until a response is decoded.
- It supports synchronous filesystem callbacks.
- `close()` closes streams and kills the child; a process-exit handler also kills tracked children.
- It is single-thread-only, which matches Oxlint's main-JS-thread callback execution.

[TSGO-SYNC-CLIENT] [TSGO-SYNC-CHANNEL]

The synchronous client rejects socket options with `Socket connections are not yet supported in the sync client`. The asynchronous client supports either a spawned process or socket and returns promises from all requests. [TSGO-SYNC-CLIENT] [TSGO-ASYNC-CLIENT]

### Project and snapshot model

The API client exposes a long-lived `API` object. `updateSnapshot` returns immutable snapshot objects containing projects, programs, checkers, and emitters. Client-side source-file materializations and remote symbol/type wrappers are cached within snapshot/project registries. Snapshots must be disposed, and `API.close()` disposes active snapshots before closing the child. [TSGO-API-CLIENT] [TSGO-SNAPSHOT-CLIENT]

On the server:

- Multiple snapshots may remain active at once.
- Queries against an existing snapshot can run while the next snapshot is being built.
- Snapshot updates are serialized to keep ref tracking and diff bases consistent.
- Open projects and files are ref-counted per API session.
- Releasing a snapshot decrements its API ref count and dereferences the project snapshot at zero.
- Closing the session releases all snapshots and all project/file refs held by that session.

[TSGO-SESSION-STATE] [TSGO-SNAPSHOT-UPDATE] [TSGO-SESSION-CLOSE]

`updateSnapshot` can open or close projects and files and can signal changed, created, deleted, or globally invalidated disk state. It does not carry arbitrary file text. `runWithTemporaryFileUpdate` creates a derived snapshot with one file's supplied text, runs a synchronous callback, and disposes the temporary snapshot in `finally`. The server implementation clones existing overlays, adds that one text overlay, and does not advance the session's latest snapshot. [TSGO-PROTOCOL] [TSGO-API-CLIENT] [TSGO-PROJECT-API]

### Query surface

The existing API is sufficient for a meaningful custom-rule prototype:

- Resolve a file's default project.
- List and materialize source files as a remote TypeScript AST.
- Fetch symbols or types at one position or a batch of positions.
- Fetch symbols or types for one node handle or a batch of node handles.
- Resolve symbol types, signatures, names, references, and many type properties.
- Request syntactic, bind, semantic, suggestion, and declaration diagnostics.
- Request edits and emit output.

[TSGO-PROJECT-PROGRAM] [TSGO-CHECKER-API]

For existing Effect rules, semantic diagnostics are the most useful starting point. If a future bridge-aware JavaScript rule needs facts not represented by those diagnostics, the client also offers batched positional methods: one `getTypesAtPositions` or `getSymbolsAtPositions` request acquires a checker once, converts each UTF-16 position, performs the loop, and returns all responses. [TSGO-CHECKER-API] [TSGO-POSITION-HANDLERS]

TypeScript-Go dedicates a persistent API checker per project. API acquisition is exclusive through a one-slot semaphore, and the checker is not idle-cleaned, preserving identity for remote type and symbol handles. A canceled persistent checker is discarded before reuse. [TSGO-CHECKER-POOL]

### Sharing an LSP project session

TypeScript-Go's LSP server implements `custom/initializeAPISession`. It creates an API session backed by the LSP server's existing `project.Session`, opens a named pipe/Unix socket, returns the session ID and pipe path, and serves that connection through `AsyncConn`. Empty API snapshot updates then adopt current LSP state. [TSGO-LSP-API] [TSGO-SNAPSHOT-UPDATE]

This is the correct source of truth for all unsaved editor documents, but it does not directly fit Oxlint's synchronous visitors:

- The LSP-created API session is asynchronous JSON-RPC.
- The official async JS client can connect to the returned socket, but every query returns a promise.
- The official sync JS client cannot connect to a socket and speaks the default MessagePack protocol to a child it starts itself.

A production bridge that shares LSP state therefore needs one of:

1. A synchronous native/socket proxy callable from the JS rule.
2. An Oxlint Rust-side bridge that performs the async exchange outside the JS visitor and supplies ready data synchronously.
3. A TypeScript-Go extension that exposes a compatible synchronous attached-session transport.
4. A prefetch phase before JavaScript traversal, which would require changing Oxlint's host protocol.

## Source text and editor correctness

### Oxlint-only known-limitation status

**Exact conclusion:** the user-visible failure is explicitly tracked, but the missing API is only implied. Open issue [#20376](https://github.com/oxc-project/oxc/issues/20376) tracks type-aware LSP diagnostics not updating when a type changes in another file; an Oxlint maintainer states that this needs the complete type context of imported/exported files for every open file and likely needs better Oxlint-to-tsgolint integration. That is direct acknowledgement of the cross-file editor failure. It does not specify an immutable all-open-document snapshot, source-overlay protocol, or JavaScript rule API as the solution. [OXC-ISSUE-LSP-CROSS-FILE]

Custom JavaScript rules have an earlier, broader limitation. Official Oxlint documentation and the JS-plugin alpha announcement say custom type-aware rules are unsupported; issues [#19596](https://github.com/oxc-project/oxc/issues/19596) and [#19962](https://github.com/oxc-project/oxc/issues/19962) respectively track exposing type information and the always-empty `parserServices`. Neither issue specifically tracks coherent unsaved multi-file state. Thus "custom typed JS rules cannot obtain a coherent snapshot of all open unsaved files" is true today, but the all-open-snapshot part is an architectural inference rather than the title or contract of an upstream issue. [OXC-DOC-JS] [OXC-BLOG-JS-ALPHA] [OXC-ISSUE-JS-TYPES] [OXC-ISSUE-PARSER-SERVICES]

The inference is direct from the inspected Oxlint revision:

- The generic language-server filesystem does contain every open document and updates/removes entries on document lifecycle events (`.repos/oxlint/crates/oxc_language_server/src/file_system.rs:12-15,77-116`; `.repos/oxlint/crates/oxc_language_server/src/backend.rs:608-719`).
- The diagnostic tool boundary receives one `TextDocument`, not that store or a snapshot of it (`.repos/oxlint/crates/oxc_language_server/src/tool.rs:97-131`).
- `ServerLinter` creates a new lint filesystem for the target, inserts only that target text, and calls `run_source` with a one-path slice; every other read falls back to disk (`.repos/oxlint/apps/oxlint/src/lsp/server_linter.rs:650-654,764-814`; `.repos/oxlint/apps/oxlint/src/lsp/lsp_file_system.rs:11-30`).
- The JavaScript callback receives one file path/source buffer plus per-file configuration, and public context exposes current-file values only (`.repos/oxlint/apps/oxlint/src-js/plugins/lint.ts:42-77,169-187`; `.repos/oxlint/apps/oxlint/src-js/plugins/context.ts:326-461`). `parserServices` is explicitly empty (`.repos/oxlint/apps/oxlint/src-js/plugins/source_code.ts:229-234`).

What can be done without changing Oxlint:

| Technique | What it can observe | Coherent? |
| --- | --- | --- |
| Current-file overlay | `context.sourceCode.text` is the exact target text. Oxlint also places that same target text in its one-file LSP filesystem, and the type-aware adapter serializes every requested path into `source_overrides`; merged PR [#14733](https://github.com/oxc-project/oxc/pull/14733) introduced this in-memory type-aware path. [OXC-PR-LSP-CURRENT-FILE] | **Only for the target file.** Imported open documents are absent from the request and therefore fall back to disk. |
| Module-level last-seen cache | A JS plugin can remember `(filename, sourceCode.text)` whenever Oxlint happens to lint a file; JS callbacks are serialized on Node's main thread. | **No.** It misses open but not-yet-linted or ignored documents, receives no open/close/delete event or document version, cannot distinguish a closed file from an unchanged open file, and mixes observations from different lint times. |
| Direct disk reads | Plugin code can read dependencies by path, and Oxlint's LSP filesystem itself falls back to disk for non-target paths. | **Disk-only, not editor-coherent.** Other unsaved buffers are invisible, and independent reads are not an atomic filesystem snapshot. |
| CLI multi-file type-aware run | `LintRunner::lint_files` passes the full selected file list with the OS filesystem, while `lint_source` reads all requested sources into one override map before spawning one type-aware run (`.repos/oxlint/crates/oxc_linter/src/lint_runner.rs:243-275`; `.repos/oxlint/crates/oxc_linter/src/tsgolint.rs:396-429`). | **Coherent enough for one ordinary disk-backed invocation, not an editor overlay.** All explicitly selected texts are frozen in one payload; dependencies outside that list remain disk-backed, and concurrent disk mutation is not transactionally excluded. JS rule callbacks remain per-file. |
| Native multi-file/module-graph analysis | Oxlint core recursively resolves imports, reads dependencies through its `RuntimeFileSystem`, links their `ModuleRecord`s, and exposes the graph to native Rust rules. [OXC-DOC-MULTI-FILE] [OXC-NATIVE-MODULE-GRAPH] [OXC-NATIVE-RULE-GRAPH] | **Not a JS-plugin workaround and not editor-coherent across files.** In LSP mode the filesystem contains only the target overlay, so graph dependencies fall back to disk; the JavaScript callback receives no module/project handle. [OXC-LSP-SOURCE] [OXC-LSP-FS] [OXC-EXTERNAL-RULE-BOUNDARY] |

There is no current Oxlint-native workaround that gives a JavaScript rule or one type-aware LSP request a point-in-time view of all open unsaved documents. The closest groundwork is merged PR [#16668](https://github.com/oxc-project/oxc/pull/16668): the lower-level lint runner accepts multiple paths and constructs a multi-file override map, while the PR explicitly says the LSP still sends one file. A future caller could populate that existing path with all relevant open files, but no first-party issue or public API found in this search commits to doing so. [OXC-PR-LSP-MULTI-FILE]

Other plans do not yet close this gap. Issue [#23211](https://github.com/oxc-project/oxc/issues/23211) is only a placeholder for type-aware language-plugin integration. Discussion [#22969](https://github.com/oxc-project/oxc/discussions/22969) proposes a persistent warm type-aware service and received maintainer agreement, but has no implementation and does not define editor overlay ingestion. Open PRs [#23796](https://github.com/oxc-project/oxc/pull/23796) and [#23797](https://github.com/oxc-project/oxc/pull/23797) add current-document versions and suppress outdated results; they prevent one freshness race but do not provide a multi-file snapshot. Workspace diagnostics issue [#16441](https://github.com/oxc-project/oxc/issues/16441) concerns requesting diagnostics for more files, not exposing all open texts to one analysis or to JavaScript rules. [OXC-ISSUE-LANGUAGE-TYPES] [OXC-DISCUSSION-WARM-TYPES] [OXC-PR-LSP-VERSIONS] [OXC-ISSUE-WORKSPACE-DIAGNOSTICS]

### What works today

Oxlint's LSP diagnostic path receives the current `TextDocument.text`, puts that text into `LspFileSystem`, and runs the normal and type-aware source pipelines against it. `LspFileSystem` falls back to disk for paths not in its map. [OXC-LSP-SOURCE] [OXC-LSP-FS]

When the native `import` plugin enables cross-module analysis, the runtime uses that same filesystem for both the lint target and every recursively resolved dependency. It retains source and semantic data for requested lint targets, processes dependencies only to construct `ModuleRecord.loaded_modules`, and then exposes that graph to native Rust rules through `LintContext::module_record()`. This is how native multi-file rules work without JavaScript `parserServices`; it does not give them a separate editor overlay. [OXC-NATIVE-MODULE-GRAPH] [OXC-NATIVE-RULE-GRAPH]

The tsgolint adapter reads each requested path through that filesystem and adds it to `source_overrides`. Tsgolint's overlay returns override text from exact-key `FileExists` and `ReadFile` lookups. An end-to-end test verifies that a disk file is analyzed using its override text. [OXC-TSGOLINT] [TGL-OVERLAY] [TGL-OVERLAY-TEST]

### What does not work completely

For one Oxlint LSP diagnostic request, only the requested document is inserted into `LspFileSystem` and only that path is sent to `tsgolint`. Imports and other source files fall back to disk. Therefore the current integration can type-check the active document's unsaved text while still seeing stale disk text for another unsaved imported document. [OXC-LSP-SOURCE] [OXC-LSP-FS] [OXC-TSGOLINT]

The native module graph has the same source-text limitation in LSP mode. If unsaved `A.ts` imports unsaved `B.ts`, a diagnostic request for `A.ts` parses `A.ts` from the one-file in-memory map but reads `B.ts` from disk. If `B.ts` exists only in the editor, normal disk-based resolution and loading cannot supply it. Requesting diagnostics for `B.ts` directly uses its editor text for that request, but `ServerLinter` creates a fresh one-file filesystem for a later request on `A.ts`, so the observation is not retained. [OXC-LSP-SOURCE] [OXC-LSP-FS] [OXC-NATIVE-MODULE-GRAPH]

Native Rust rules can consume the resulting `ModuleRecord` graph. JavaScript rules cannot: the external-rule callback carries the current path, rule/options IDs, settings, globals, workspace URI, and current-program allocator, but no module graph, project, or filesystem handle; `SourceCode.parserServices` remains empty. [OXC-NATIVE-RULE-GRAPH] [OXC-EXTERNAL-RULE-BOUNDARY] [OXC-PARSER-SERVICES]

The command-local tsgolint overlay also only overrides exact `FileExists` and `ReadFile` calls. `DirectoryExists`, `GetAccessibleEntries`, `Stat`, and `WalkDir` delegate to disk, so a purely virtual file may not participate correctly in directory discovery or glob/config expansion. Tsgolint has a separate fuller `OverlayVFS` that synthesizes directories and entries for virtual files, which highlights the missing behavior in the headless overlay. [TGL-OVERLAY] [TGL-FULL-OVERLAY]

A standalone synchronous TypeScript-Go child has the same fundamental information problem: an Oxlint JS rule sees its current context, not the LSP server's entire open-document store. Passing `context.sourceCode.text` through a temporary snapshot fixes the current file only. It cannot guarantee a coherent project view when multiple imported files have unsaved changes.

The standalone API server also does not install filesystem watchers. Its persistent project state advances only when the client calls `updateSnapshot` and supplies opened/closed files or a changed/created/deleted/global invalidation summary. The broker must therefore feed observed disk changes, conservatively invalidate when needed, or accept that imported disk files can also become stale in a long-lived Oxlint LSP process. Always applying the current-file temporary overlay does not solve dependency invalidation. [TSGO-SERVER] [TSGO-PROTOCOL]

### Where Oxlint's complete editor overlay actually lives

Oxlint has two distinct LSP filesystem layers. The generic Rust LSP backend owns a server-wide concurrent map from every open document URI to its language ID and `Arc<str>` text. `didOpen` and full-document `didChange` replace entries, while `didClose` removes them. The map does not retain LSP document versions. [OXC-OPEN-DOCUMENTS] [OXC-DOCUMENT-LIFECYCLE]

That complete map is not passed to the linter tool. The `Tool` diagnostic interface receives one `TextDocument`, and `ServerLinter` creates a new ephemeral `LspFileSystem` containing only that target path/text before calling `LintRunner::run_source` with a one-element path list. The local filesystem falls back to disk for every other path. [OXC-TOOL-DOCUMENT] [OXC-LSP-SOURCE] [OXC-LSP-FS]

Tsgolint consequently does not receive all editor overlays today. Its adapter constructs `source_overrides` by reading only the paths requested for that lint run; in the LSP path that is the same one target file. [OXC-TSGOLINT]

The JavaScript plugin callback is similarly current-file-only. Its host arguments are the current file path, current source/AST buffer, enabled rule IDs/options, settings, globals, and an internal workspace URI. Public rule context exposes the current filename, CWD, `SourceCode`, settings, and language options, but no open-document collection, filesystem, LSP version, event kind, or close notification. [OXC-JS-LINT-CALL] [OXC-JS-CONTEXT]

Therefore a custom plugin cannot access a coherent complete editor overlay through current Oxlint JavaScript APIs. A plugin can copy `context.sourceCode.text` into a module map whenever it happens to lint a file, but that map misses ignored or not-yet-linted documents, has no close/delete signal, has no version/order information, and retains stale text after a document closes. It is useful only as a best-effort experiment.

### Recommended multi-file overlay transport

Do not enumerate or copy every open source text in each rule's `before` hook. Keep the editor overlay source of truth at the Oxlint host boundary and expose it to the broker incrementally or lazily.

The most robust interface is a point-in-time, read-only workspace overlay snapshot supplied before JavaScript rule execution:

- A monotonic snapshot revision.
- File metadata containing path/URI, language ID, and per-file revision, without eagerly copying text.
- Opened, changed, and closed paths since the previous revision, or an `invalidateAll` marker when the delta is unavailable.
- Synchronous lazy methods such as `readFile`, `fileExists`, `directoryExists`, `getAccessibleEntries`, and `realpath` backed by immutable `Arc<str>` document contents.

Oxlint can create such a snapshot cheaply by cloning path metadata and `Arc` references from its existing Rust map under a brief lock. A N-API provider object can expose lazy reads while the lint callback is active, avoiding serialization of every text on every file. The provider must be immutable for that lint invocation so a TypeScript snapshot cannot mix editor revisions.

An initial full snapshot followed by ordered `didOpen`/`didChange`/`didClose` deltas is a simpler alternative if Oxlint prefers lifecycle hooks over a lazy native provider. It must include a workspace generation and monotonic event sequence to handle concurrent diagnostics and the existing same-URI workspace reload race. Seed once when the worker/plugin workspace is created, then send only changed text and close records.

On the TypeScript-Go side, no new VFS mechanism is needed. Construct the synchronous `API` with filesystem callbacks backed by the current Oxlint overlay provider. Returning `undefined` delegates a path to the real filesystem; returning overlay text or `true` shadows disk. The callback filesystem already supports the operations needed for normal project lookup, and the client services callbacks inline while blocked on the TypeScript-Go request. [TSGO-FS-API] [TSGO-SYNC-CLIENT] [TSGO-CALLBACK-FS]

The callbacks are overrides, not an automatic union with disk. For a directory containing virtual children, `getAccessibleEntries` must merge those children with real `readdir` results rather than returning only the overlay entries. `directoryExists`, `fileExists`, `readFile`, and `realpath` should return `undefined` when the overlay has no opinion. The current callback layer does not virtualize `Stat` or `WalkDir`; TypeScript-Go's existing tests cover changed and newly created source files, but a fully synthetic directory/package graph may require narrowly extending those callbacks. [TSGO-FS-API] [TSGO-CALLBACK-FS]

Before querying diagnostics, coalesce Oxlint overlay deltas into one persistent snapshot update:

1. Apply opened/changed/closed records to the broker's overlay metadata.
2. Call `updateSnapshot` with `openFiles`, `closeFiles`, and precise `fileChanges.changed`, `created`, or `deleted` paths.
3. Let TypeScript-Go lazily call back for the contents it actually needs while rebuilding affected projects.
4. Dispose the previous base snapshot after the new one is ready.
5. Run the shared Effect diagnostic request directly on that coherent base snapshot.

TypeScript-Go's tests demonstrate that mutating a callback-backed VFS and sending a precise file-change summary creates a new snapshot with the changed content while preserving unchanged source-file identities. [TSGO-FS-SNAPSHOTS]

With this model, `runWithTemporaryFileUpdate` is no longer needed for ordinary TypeScript/JavaScript editor files because the base snapshot already contains the current target and every open dependency at one overlay revision. It remains useful as a fallback for transformed framework sections or synthetic sources that do not correspond one-to-one with the editor document.

The easiest but least desirable Oxlint patch would pass a complete `path -> text` map on every JavaScript lint callback. It is correct for a point in time but copies all open text once per target file, exactly the cost the lazy or delta interface avoids.

### Required source-of-truth policy

The bridge should make the following policy explicit rather than silently mixing states:

| Mode | Required source |
| --- | --- |
| CLI | Disk snapshot, with a temporary override when Oxlint's current text differs from disk or comes from a virtual source. |
| LSP, standalone child | Current file override plus disk dependencies; diagnostics must be documented as potentially stale across unsaved dependencies. |
| LSP, full correctness | Add an Oxlint host snapshot/delta interface backed by its existing open-document map, or attach to TypeScript-Go's LSP API session. |

## Recommended bridge design

### Phase 1: diagnostic-only plugin, no Oxlint changes

Build the broker and generated plugin rules into `@effect/tsgo` or a companion package. Keep one broker per workspace/CWD and lazily create the sync API on the first eligible TypeScript file. Resolve the patched executable through `@effect/tsgo/lib/getExePath`; the repository already uses executable metadata to ensure that the packaged binary matches the installed native TypeScript revision. The matching unstable API client must also be bundled because the inspected `@typescript/native-preview` package is private and the sync protocol is unversioned. [EFFECT-EXE-PATH] [TSGO-PACKAGE] [TSGO-SYNC-CHANNEL]

Generate the Oxlint rule map from Effect metadata. Each generated rule uses `createOnce` with three operations:

1. `Program` registers the rule name, context, and current Oxlint options in a per-file broker frame.
2. `Program:exit` calls `resultsForCurrentFile()`, then reports only the diagnostic bucket for that wrapper's rule name. The first exit computes after all entries have registered; later exits reuse the result.
3. The last `Program:exit` clears the frame. `after` performs only idempotent cleanup if traversal failed before normal completion.

For the first request in a frame, the broker:

1. Opens the file in a persistent base snapshot if its project is not already available.
2. Keeps only the newest base snapshot and disposes replaced snapshots.
3. Calls `runWithTemporaryFileUpdate(base, file, context.sourceCode.text, callback)` so the exact Oxlint text is authoritative.
4. In the callback, resolves the file's default project and calls `project.program.getSemanticDiagnostics(file)` exactly once.
5. Filters Effect codes, flattens message chains, maps codes to names, and groups the results.
6. Reports through synthetic ranged objects such as `{ range: [diag.pos, diag.end] }` before returning from the synchronous `Program:exit` visitor.

This first phase intentionally uses the existing `tsconfig` Effect configuration and provides diagnostics only. It proves process reuse, snapshot reuse, in-memory current-file text, fan-out, and overhead without a TypeScript-Go patch or an Oxlint patch.

### Phase 2: Effect-specific lint endpoint

Once the proof is fast enough, add one narrow request to the patched TypeScript-Go API, for example `getEffectDiagnostics`. It should accept the file, selected Effect rule names, and Effect lint options, then invoke the existing Go runner once and return diagnostics grouped by rule. This avoids running disabled Oxlint rules and makes Oxlint configuration the source of truth instead of requiring duplicate `tsconfig` enablement.

If fix parity is in scope, the same response should optionally carry current-file text edits and alternative suggestions. This deliberately mirrors the useful shape of tsgolint's headless response while retaining the persistent TypeScript-Go session. Do not expose raw checker objects or reimplement each Effect rule in JavaScript.

The endpoint should also define which suppression system owns the result. The clean Oxlint model is to let Oxlint apply `oxlint-disable` directives and rule severity, while the Go endpoint applies Effect rule logic and options. Reapplying `@effect-diagnostics` severity transformations would be confusing because a JavaScript `context.report` cannot override the Oxlint-configured severity per occurrence.

### Phase 3: editor-state and host integration

Only after the standalone plugin is useful should integration expand to coherent multi-file editor overlays. The preferred next step is an Oxlint host change that exposes an immutable lazy snapshot or ordered deltas from its existing Rust open-document map. Back the standalone synchronous TypeScript-Go client's filesystem callbacks with that provider and advance persistent snapshots using precise file-change summaries.

Attaching to a TypeScript-Go LSP-backed API remains an alternative when both tools can share the same language-server session, but it still needs an Oxlint native proxy or prefetch phase because the attached API transport is asynchronous while JavaScript visitors are synchronous.

Replacing Oxlint's empty `parserServices` or emulating typescript-eslint node maps is not required for existing Effect diagnostics and should not be bundled into this work. It is a separate compatibility project.

## Lifecycle constraints

Oxlint has logical JS workspaces keyed by URI and switches global singleton state between them. Workspace creation replaces stored rule/config state for that URI. Workspace destruction is currently a no-op because reload events can arrive in create-then-destroy order for the same URI, and the LSP builder's shutdown method correspondingly returns without invoking destruction. [OXC-WORKSPACE] [OXC-WORKSPACE-SHUTDOWN]

There is also no plugin-level destructor in the plugin interface. Therefore a plugin-only prototype cannot reliably close one TypeScript process exactly when an Oxlint workspace is removed. [OXC-PLUGIN-SHAPE]

For the prototype, a process-scope broker map keyed by workspace/CWD is the least surprising policy:

- Start each workspace child at its first typed query.
- Reuse each child through the entire Oxlint Node process.
- Dispose snapshots eagerly after replacement.
- Register explicit process-exit cleanup and rely on the official sync channel's live-child cleanup as a final fallback.
- Treat workspace reload as cache invalidation, not process termination.

For production, fix workspace lifecycle with a generation/token rather than enabling the unreachable deletion as written. A destroy request must identify the specific workspace incarnation so a late destroy cannot remove its replacement.

## Performance analysis

### Main cost centers

1. **Serialized JS section.** Every blocking RPC extends a callback that already runs on Node's main thread and blocks a Rust lint worker. [OXC-CALLBACK]
2. **Snapshot updates.** Project graph refresh and changed-file processing can dominate a small rule's own logic. Updates are serialized server-side. [TSGO-SNAPSHOT-UPDATE]
3. **Checker exclusivity.** API operations for a project serialize on its persistent checker. [TSGO-CHECKER-POOL]
4. **Temporary snapshot payload.** A temporary update returns refreshed project metadata before the semantic request; this can transfer much more data than the diagnostics themselves.
5. **Duplicate work across wrappers.** Independent one-to-one wrappers would repeat the same snapshot and semantic-diagnostic requests.
6. **Unselected Go rules.** The generic endpoint runs whatever the Effect `tsconfig` enables, even if Oxlint enabled only one wrapper.

### Required optimization strategy

- Run one shared semantic-diagnostics request from a guaranteed `Program` visitor.
- Group the response by stable Effect rule code and fan it out in JavaScript.
- Never fetch or materialize the TypeScript AST for existing Effect rules.
- Add the selected-rule Effect endpoint if running all `tsconfig`-enabled rules is material in profiles.
- Reuse the persistent process and project state, but release stale snapshots promptly.
- Share the temporary snapshot and diagnostic result among all wrappers for the same file invocation.
- Use TypeScript-Go's built-in `--timing`/client timing collection during experiments. Both server and client already collect per-request timing and transferred payload sizes. [TSGO-CLI] [TSGO-SYNC-CLIENT]

The local proof measured a warm semantic request for 94 enabled Effect rules at 3.43 ms on the small `floatingEffect.ts` fixture, with 157 request bytes and 885 response bytes. The initial project load took most of the first-run time, and a temporary snapshot response was about 42 KB for that fixture. These numbers justify building the plugin proof, but not skipping representative monorepo, repeated-LSP, and memory benchmarks.

## Failure and compatibility risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Promise-returning bridge used in a visitor | Results and diagnostics arrive after Oxlint reset the file context | Expose synchronous methods only; reject thenables in bridge wrappers during development. |
| Client/binary protocol drift | Decode failure or incorrect handles | Build and pin client plus binary from one TypeScript-Go revision; sync protocol is explicitly unversioned. [TSGO-SYNC-CHANNEL] |
| Child crash or broken pipe | Current lint file fails; singleton remains poisoned | Detect transport failure, close the client, invalidate snapshots, allow one clean restart, and report a stable Oxlint diagnostic. |
| Stale imported editor file | Incorrect type facts | Attach to LSP session for production editor support or clearly limit standalone mode. |
| Snapshot leak | Growing Go and JS memory | Structured `try/finally`, one retained base snapshot, explicit `dispose`, and `API.close`. |
| Workspace reload race | Wrong process/cache teardown | Use workspace generations; do not rely on current no-op destroy. |
| Wrong project for a file | Incorrect compiler options and module graph | Open the file/project, then use `getDefaultProjectForFile`; do not guess by nearest tsconfig in JS. |
| UTF-8/UTF-16 mismatch | Querying the wrong TypeScript node | Use Oxlint node ranges directly as UTF-16 offsets and include astral-character tests. |
| Virtual file absent from directory listings | Project discovery misses it | Use TypeScript-Go temporary overlays for known files and add complete directory semantics if arbitrary virtual files are required. |
| Oxlint rule enabled but `tsconfig` rule disabled | Missing diagnostic in the generic-endpoint prototype | Document the shared-config limitation, then add a selected-rule Effect endpoint for production. [EFFECT-RUNNER] |
| Generic diagnostic response has no fixes | Fixable metadata cannot produce an Oxlint autofix | Ship diagnostics first or add current-file edits to the Effect endpoint. [TSGO-DIAGNOSTIC-SHAPE] [EFFECT-FIXES] |
| Alpha/unstable API changes | Ongoing maintenance | Pin versions, keep the bridge small, and maintain conformance tests against both sides. [OXC-DOC-JS] [TSGO-PACKAGE] |

## Proof-of-concept plan and go/no-go tests

### Test rule

Generate wrappers for `floatingEffect` and one second rule with a different code. This proves result fan-out and enabled-wrapper coordination while using the exact existing Go implementations. Keep fixes out of the first proof.

### Experiments

1. **Persistence:** lint many files and prove exactly one `tsgo --api` child is created.
2. **Snapshot correctness:** change a file on disk between lint requests and prove the next base snapshot sees it.
3. **Current unsaved file:** lint text that differs from disk and prove the temporary snapshot sees the supplied text.
4. **Unsaved dependency:** change an imported file only in the editor and demonstrate the known standalone limitation; repeat against a shared LSP session when a synchronous attachment exists.
5. **Range fidelity:** test ASCII, BMP non-ASCII, astral characters before the diagnostic, BOMs, and zero-width ranges.
6. **Project selection:** test configured, inferred, project-reference, monorepo, JavaScript, JSX, and TSX files.
7. **Fan-out:** enable many wrappers and prove one temporary update and one semantic request occur per file, independent of wrapper count and order.
8. **Lifecycle:** repeatedly reload an LSP workspace and verify process, snapshot, and memory counts stabilize.
9. **Failure recovery:** kill the child during a request and verify one bounded failure followed by clean reinitialization.
10. **Rule selection:** compare the generic all-enabled pass with an Effect endpoint that runs only the Oxlint-enabled rules.
11. **Configuration:** verify and document behavior when Oxlint, `tsconfig`, `oxlint-disable`, and `@effect-diagnostics` disagree.

### Go criteria

- One process survives across files and repeated LSP diagnostics.
- Current-file unsaved content is always used.
- Every generated wrapper maps only its own diagnostics while the broker issues one semantic request.
- All range tests pass without rule-specific offset corrections.
- The shared semantic pass has acceptable end-to-end overhead on representative projects.
- Snapshot and memory counts stabilize under repeated updates.
- Protocol mismatch and process failure produce deterministic diagnostics rather than hangs.
- The generated rules contain no semantic logic and expose no transport details.

### No-go or redesign criteria

- The rule requires promises or background completion after a visitor returns.
- Correctness requires all unsaved dependency files but no shared LSP session or multi-file overlay can be supplied.
- The shared semantic pass spends unacceptable time in the serialized JavaScript callback on representative projects.
- Production configuration or fix parity requires broad checker exposure instead of one narrow lint response.

## Alternatives considered

### Keep custom typed rules in Go/tsgolint

This remains the best performance and integration path for rules intended to ship as built-in Oxlint type-aware rules. It has direct TypeScript AST/checker access, checker-parallel workers, and the existing diagnostic protocol. Its downside is that rules are compiled Go code and cannot be distributed as normal JavaScript plugins. [TGL-LINTER] [TGL-RULE-API]

### Use the asynchronous TypeScript-Go client directly

Rejected for current Oxlint visitors. The client is suitable for standalone tools and socket attachment, but Oxlint's generated walkers do not await visitor results. [TSGO-ASYNC-CLIENT] [OXC-VISITORS]

### Spawn tsgolint for each JavaScript query

Rejected. It repeats process startup and program construction, and tsgolint does not execute JS rule logic. [OXC-TSGOLINT] [TGL-WORKLOAD]

### Materialize the complete TypeScript AST and emulate parser services

Possible but unnecessary for existing Effect diagnostics. It adds transfer/materialization cost and still requires building reliable bidirectional maps between Oxc ESTree nodes and TypeScript nodes. The semantic-diagnostics fan-out answers the actual requirement with much less machinery. [TSGO-PROJECT-PROGRAM]

### Reimplement Effect rules as positional JavaScript rules

Rejected for the existing rule set. Batched positional checker APIs are useful for new custom behavior, but rewriting semantic logic would duplicate the Go implementations, increase RPC count, and risk behavior drift. Keep the rules in Go and move only their diagnostic results across the bridge.

## Final recommendation

Proceed with a diagnostic-only generated Oxlint plugin backed by one synchronous `tsgo --api` process per workspace. Reuse `getSemanticDiagnostics(file)` first, fan out by the existing code-to-rule metadata, and always supply Oxlint's current source text through a temporary snapshot. This path is already proven against the patched checker and requires no Oxlint changes and no new TypeScript-Go API method.

Treat that as the feasibility prototype, not the final configuration contract. If performance is acceptable, add one narrow `getEffectDiagnostics` request that accepts selected rule names/options and can later return current-file fixes. Copy tsgolint's per-file rule/options and structured-diagnostic ideas, but retain TypeScript-Go's persistent process and snapshot model.

For CLI use, the standalone child can plausibly be the final architecture. For production editor use with coherent unsaved dependencies, plan a second integration that shares TypeScript-Go LSP state or supplies a complete multi-file overlay. Do not begin by implementing `parserServices`, adapting arbitrary typed ESLint plugins, or translating Effect rule logic into JavaScript.

Any TypeScript-Go changes belong in minimal patches under `_patches`; direct edits to `typescript-go` are reset by validation. [REPO-POLICY]

## Primary sources

[OXC-DOC-JS]: https://oxc.rs/docs/guide/usage/linter/js-plugins.html "Official Oxlint JS plugin documentation: alpha status, API support, and typed-rule limitation"
[OXC-DOC-CREATE-ONCE]: https://oxc.rs/docs/guide/usage/linter/writing-js-plugins.html#before-hook "Official alternative-API documentation: before-hook non-guarantee, planned node-interest optimization, and Program recommendation"
[OXC-BLOG-JS-ALPHA]: https://oxc.rs/blog/2026-03-11-oxlint-js-plugins-alpha.html#what-it-can-t-do-yet "Official announcement: no custom type-aware rules"
[OXC-DOC-MULTI-FILE]: https://oxc.rs/docs/guide/usage/linter/multi-file-analysis.html "Official Oxlint documentation: native project-wide module-graph analysis"
[OXC-ISSUE-JS-TYPES]: https://github.com/oxc-project/oxc/issues/19596 "Open tracker for making type information available to custom JavaScript plugins"
[OXC-ISSUE-PARSER-SERVICES]: https://github.com/oxc-project/oxc/issues/19962 "Open tracker for empty parserServices in JavaScript plugins"
[OXC-ISSUE-LSP-CROSS-FILE]: https://github.com/oxc-project/oxc/issues/20376#issuecomment-4060523219 "Open type-aware LSP cross-file update bug and maintainer acknowledgement of required complete open-file context"
[OXC-PR-LSP-CURRENT-FILE]: https://github.com/oxc-project/oxc/pull/14733 "Merged support for sending the current in-memory LSP source to type-aware linting"
[OXC-PR-LSP-MULTI-FILE]: https://github.com/oxc-project/oxc/pull/16668 "Merged multi-path lint-runner groundwork; LSP remains single-file"
[OXC-ISSUE-LANGUAGE-TYPES]: https://github.com/oxc-project/oxc/issues/23211 "Placeholder for type-aware language-plugin integration"
[OXC-DISCUSSION-WARM-TYPES]: https://github.com/oxc-project/oxc/discussions/22969#discussioncomment-17190243 "Proposed persistent warm type-aware service and maintainer response"
[OXC-PR-LSP-VERSIONS]: https://github.com/oxc-project/oxc/pull/23796 "Open document-version PR; outdated-result suppression is stacked in PR #23797"
[OXC-ISSUE-WORKSPACE-DIAGNOSTICS]: https://github.com/oxc-project/oxc/issues/16441 "Open workspace-wide diagnostics tracker"
[OXC-PR-BEFORE-EMPTY]: https://github.com/oxc-project/oxc/pull/14401 "Merged hooks-only suppression and rationale for the before-hook non-guarantee"
[OXC-PLUGIN-SHAPE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/load.ts#L15-L45 "Plugin and rule interfaces"
[OXC-PLUGIN-IMPORT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/load.ts#L108-L132 "Asynchronous module import"
[OXC-PLUGIN-LOAD]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/load.ts#L249-L305 "Context creation and createOnce registration"
[OXC-CONTEXT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/context.ts#L11-L26 "Reused rule contexts and singleton file context"
[OXC-LINT-FILE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/lint.ts#L169-L259 "Per-file visitor compilation and execution"
[OXC-BEFORE-REGISTER]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/load.ts#L268-L304 "createOnce registration and deliberate replacement of hooks-only visitors"
[OXC-BEFORE-RUN]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/lint.ts#L157-L259 "Indeterminate rule order, before-hook invocation, false return, visitor compilation, traversal, and after hooks"
[OXC-BEFORE-ERROR-TEST]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/test/fixtures/createOnce_hook_errors/plugin.ts#L75-L178 "Fixture proving an earlier before exception prevents a later rule's hook while completed rules receive cleanup"
[OXC-VISITORS]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/generated/walk.js#L640-L675 "Synchronous hook invocation; hook return types are in apps/oxlint/src-js/plugins/types.ts lines 8-19"
[OXC-PROGRAM-WALK]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/generated/walk.js#L1481-L1495 "Combined Program enter callbacks, child traversal, and Program exit callbacks"
[OXC-PARSER-SERVICES]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/source_code.ts#L215-L234 "Empty parser services"
[OXC-CALLBACK]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src/js_plugins/external_linter.rs#L148-L228 "Main-thread ThreadsafeFunction and blocking caller"
[OXC-PARALLEL]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/service/runtime.rs#L354-L378 "Parallel native file processing"
[OXC-AST-SPANS]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/lib.rs#L720-L749 "AST, token, and comment conversion to UTF-16"
[OXC-JS-DIAGNOSTICS]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/lib.rs#L810-L849 "JS diagnostic and fix conversion back to UTF-8"
[OXC-REPORT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/report.ts#L16-L36 "JavaScript diagnostics accept a node or explicit location; ranged reporting is handled at lines 121-200"
[OXC-LINT-SEQUENCE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/lint_runner.rs#L243-L299 "Regular then type-aware lint ordering"
[OXC-NATIVE-NODE-SKIP]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/lib.rs#L380-L400 "Implemented AST-node-interest skip for native Rust rules; external rule dispatch is separate at lines 513-524"
[OXC-RULE-ENABLEMENT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/config/config_store.rs#L85-L106 "Disabled external-rule filtering; per-file override filtering is at lines 126-151 and 230-280"
[OXC-FILE-SELECTION]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src/walk.rs#L132-L146 "CLI path, extension, and minified-file filtering; ignore filtering is in apps/oxlint/src/lint.rs lines 357-368"
[OXC-PARSE-GATE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/service/runtime.rs#L962-L1060 "File read and entry/dependency processing; parse and semantic failures are handled at lines 1123-1159"
[OXC-TSGOLINT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/tsgolint.rs#L366-L524 "One payload, child creation, diagnostic stream, and cleanup"
[OXC-LSP-SOURCE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src/lsp/server_linter.rs#L650-L830 "TextDocument contents and single-file LSP filesystem population"
[OXC-LSP-FS]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src/lsp/lsp_file_system.rs#L10-L35 "In-memory file lookup with disk fallback"
[OXC-OPEN-DOCUMENTS]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_language_server/src/file_system.rs#L12-L117 "Server-wide open-document map and accessors"
[OXC-DOCUMENT-LIFECYCLE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_language_server/src/backend.rs#L572-L741 "Open, full-change, save, and close handling for editor documents"
[OXC-TOOL-DOCUMENT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_language_server/src/tool.rs#L97-L132 "Current single-document diagnostic interface"
[OXC-JS-LINT-CALL]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/lint.ts#L42-L77 "Rust-to-JavaScript lint callback arguments"
[OXC-JS-CONTEXT]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/plugins/context.ts#L326-L488 "Public current-file JavaScript rule context"
[OXC-NATIVE-MODULE-GRAPH]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/service/runtime.rs#L170-L182 "Shared runtime filesystem and recursive native module-graph construction; dependency processing continues at lines 323-580 and 962-1179"
[OXC-NATIVE-RULE-GRAPH]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/context/mod.rs#L73-L92 "Native Rust rule access to the current module record and its linked graph"
[OXC-EXTERNAL-RULE-BOUNDARY]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/crates/oxc_linter/src/external_linter.rs#L43-L64 "External JavaScript rule callback arguments; current-program transfer occurs in crates/oxc_linter/src/lib.rs lines 787-822"
[OXC-WORKSPACE]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src-js/workspace/index.ts#L23-L92 "Workspace singleton state and no-op destruction"
[OXC-WORKSPACE-SHUTDOWN]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src/lsp/server_linter.rs#L288-L306 "Disabled workspace shutdown"
[OXC-NODE-RUNTIME]: https://github.com/oxc-project/oxc/blob/a065946a8ce95eb3374e08242cd9086ab050314b/apps/oxlint/src/run.rs#L167-L223 "Node/N-API external linter construction and platform gate"

[TGL-HEADLESS]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/cmd/tsgolint/headless.go#L195-L258 "Output framing, one-shot stdin read, and filesystem setup"
[TGL-PAYLOAD]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/cmd/tsgolint/payload.go#L10-L83 "V1/V2 payloads and source overrides"
[TGL-WORKLOAD]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/internal/linter/linter.go#L80-L210 "Program creation per workload; assignment is in cmd/tsgolint/headless.go lines 260-327"
[TGL-LINTER]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/internal/linter/linter.go#L368-L619 "Checker workloads, workers, listeners, and traversal"
[TGL-RULE-API]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/internal/rule/rule.go#L32-L130 "Go rule/listener, diagnostic, and checker context shape"
[TGL-RULES]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/cmd/tsgolint/main.go#L162-L230 "Compile-time rule registry"
[TGL-OVERLAY]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/cmd/tsgolint/overlayfs.go#L9-L73 "Exact file overrides and disk-delegated directory operations"
[TGL-FULL-OVERLAY]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/internal/utils/overlay_vfs.go#L14-L75 "Virtual directory and entry synthesis"
[TGL-OVERLAY-TEST]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/e2e/snapshot.test.ts#L427-L495 "Source override end-to-end tests"
[TGL-SHIMS]: https://github.com/oxc-project/tsgolint/blob/16a224c6cc96e4111cc6edfeded8e3028c2b59ce/tools/gen_shims/README.md#L1-L24 "Generated shim design, reexports, and go:linkname"

[TSGO-CLI]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/cmd/tsgo/api.go#L17-L60 "API CLI mode and options"
[TSGO-SERVER]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/server.go#L14-L124 "Server options, transport, project session, and protocol selection"
[TSGO-CONN-SYNC]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/conn_sync.go#L16-L80 "Serial synchronous connection"
[TSGO-CONN-ASYNC]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/conn_async.go#L17-L86 "Concurrent asynchronous connection"
[TSGO-PACKAGE]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/package.json#L1-L84 "Preview package status and unstable exports"
[TSGO-SYNC-CLIENT]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/sync/client.ts#L20-L161 "Spawn-only synchronous client, callbacks, timing, and close"
[TSGO-SYNC-CHANNEL]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/syncChannel.ts#L1-L266 "Blocking pipe I/O, cleanup, protocol coupling, thread constraint, and close"
[TSGO-FS-API]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/fs.ts#L1-L152 "JavaScript callback filesystem contract and virtual filesystem implementation"
[TSGO-CALLBACK-FS]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/callbackfs.go#L12-L197 "Server-side filesystem delegation with real-filesystem fallback"
[TSGO-FS-SNAPSHOTS]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/test/sync/api.test.ts#L776-L867 "Callback-backed VFS changed/created snapshot tests"
[TSGO-ASYNC-CLIENT]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/async/client.ts#L34-L195 "Async spawn/socket modes and promise request API"
[TSGO-API-CLIENT]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/sync/api.ts#L148-L269 "API initialization, snapshots, close, and temporary file updates"
[TSGO-SNAPSHOT-CLIENT]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/sync/api.ts#L320-L514 "Snapshot lookup, disposal, and object registries"
[TSGO-PROTOCOL]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/proto.ts#L108-L170 "Open/close, file changes, and temporary text parameters"
[TSGO-PROJECT-PROGRAM]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/sync/api.ts#L670-L829 "Project, program, and remote source-file materialization"
[TSGO-DIAGNOSTICS-API]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/sync/api.ts#L913-L975 "Synchronous program diagnostic methods; server handler is internal/api/session.go lines 3349-3368"
[TSGO-DIAGNOSTIC-SHAPE]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/proto.go#L1247-L1321 "Diagnostic response fields and UTF-8-to-UTF-16 conversion"
[TSGO-CHECKER-API]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/_packages/native-preview/src/api/sync/api.ts#L1074-L1292 "Checker, symbol, type, and batched position APIs"
[TSGO-SESSION-STATE]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/session.go#L359-L483 "Concurrent snapshots, serialized updates, lookup, and release"
[TSGO-SNAPSHOT-UPDATE]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/session.go#L877-L1093 "Persistent and temporary snapshot updates, ref tracking, and release"
[TSGO-SESSION-CLOSE]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/session.go#L3222-L3264 "Session cleanup"
[TSGO-PROJECT-API]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/project/api.go#L12-L68 "Persistent API updates and one-file temporary overlays"
[TSGO-POSITION-HANDLERS]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/api/session.go#L1290-L1611 "Symbol and type position conversion and batching"
[TSGO-CHECKER-POOL]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/project/checkerpool.go#L22-L342 "Checker categories, persistent API checker, and exclusive acquisition"
[TSGO-LSP-API]: https://github.com/microsoft/typescript-go/blob/70c2f5e51856a908b05ac98b5e954b4c685520dd/internal/lsp/server.go#L1759-L1824 "LSP-backed API session, async pipe, session ID, and pipe result"

[EFFECT-CHECKER-PATCH]: https://github.com/Effect-TS/tsgo/blob/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2/_patches/002-checker-checker.patch#L54-L71 "Patched after-check callback invocation"
[EFFECT-HOOK]: https://github.com/Effect-TS/tsgo/blob/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2/etscheckerhooks/init.go#L17-L41 "Effect checker hook registration and diagnostic emission"
[EFFECT-RUNNER]: https://github.com/Effect-TS/tsgo/blob/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2/internal/rulerunner/diagnostics.go#L28-L145 "Per-file runner, selected rule names, shared type parser, and rule execution"
[EFFECT-METADATA]: https://github.com/Effect-TS/tsgo/blob/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2/_packages/tsgo/src/metadata.json#L54-L100 "Generated rule names, descriptions, severities, fixability, supported versions, and codes"
[EFFECT-FIXES]: https://github.com/Effect-TS/tsgo/blob/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2/etslshooks/init.go#L84-L135 "Effect language-service code-fix provider"
[EFFECT-EXE-PATH]: https://github.com/Effect-TS/tsgo/blob/c9b44998eaf5c8df0f18a1bdb8dc95376d259de2/_packages/tsgo/lib/getExePath.js#L6-L72 "Resolution and revision matching for packaged patched executables"
[REPO-POLICY]: AGENTS.md#L1-L6 "Repository rules for TypeScript-Go shims and patches"
