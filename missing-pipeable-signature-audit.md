# Missing Pipeable Signature Audit

The `missingPipeableSignature` diagnostic reported these 87 entries when run against the Effect Smol root project. Paths are relative to `.repos/effect-smol`.

| # | Export | Source |
|---:|---|---|
| 1 | `handler` | `ai-docs/src/51_http-server/10_basics.ts:60` |
| 2 | `model` | `packages/ai/openai-compat/src/OpenAiEmbeddingModel.ts:50` |
| 3 | `model` | `packages/ai/openai-compat/src/OpenAiLanguageModel.ts:317` |
| 4 | `resolveFinishReason` | `packages/ai/openai-compat/src/internal/utilities.ts:21` |
| 5 | `make` | `packages/ai/openrouter/src/Generated.ts:8885` |
| 6 | `OpenRouterClientError` | `packages/ai/openrouter/src/Generated.ts:10018` |
| 7 | `model` | `packages/ai/openrouter/src/OpenRouterLanguageModel.ts:233` |
| 8 | `useAtomValue` | `packages/atom/react/src/Hooks.ts:80` |
| 9 | `useAtomSet` | `packages/atom/react/src/Hooks.ts:144` |
| 10 | `useAtom` | `packages/atom/react/src/Hooks.ts:182` |
| 11 | `useAtomSuspense` | `packages/atom/react/src/Hooks.ts:249` |
| 12 | `useAtomSubscribe` | `packages/atom/react/src/Hooks.ts:268` |
| 13 | `useAtomRefProp` | `packages/atom/react/src/Hooks.ts:294` |
| 14 | `useAtomRefPropValue` | `packages/atom/react/src/Hooks.ts:301` |
| 15 | `useAtomValue` | `packages/atom/solid/src/Hooks.ts:40` |
| 16 | `useAtomSet` | `packages/atom/solid/src/Hooks.ts:113` |
| 17 | `useAtom` | `packages/atom/solid/src/Hooks.ts:150` |
| 18 | `useAtomSubscribe` | `packages/atom/solid/src/Hooks.ts:176` |
| 19 | `useAtomResource` | `packages/atom/solid/src/Hooks.ts:191` |
| 20 | `useAtomRefProp` | `packages/atom/solid/src/Hooks.ts:228` |
| 21 | `useAtomRefPropValue` | `packages/atom/solid/src/Hooks.ts:237` |
| 22 | `useAtom` | `packages/atom/vue/src/index.ts:85` |
| 23 | `useAtomSet` | `packages/atom/vue/src/index.ts:162` |
| 24 | `match` | `packages/effect/src/internal/concurrency.ts:7` |
| 25 | `matchSimple` | `packages/effect/src/internal/concurrency.ts:31` |
| 26 | `addError` | `packages/effect/src/unstable/eventlog/Event.ts:325` |
| 27 | `EntryIdOrder` | `packages/effect/src/unstable/eventlog/EventJournal.ts:166` |
| 28 | `group` | `packages/effect/src/unstable/eventlog/EventLog.ts:431` |
| 29 | `groupCompaction` | `packages/effect/src/unstable/eventlog/EventLog.ts:461` |
| 30 | `groupReactivity` | `packages/effect/src/unstable/eventlog/EventLog.ts:552` |
| 31 | `layer` | `packages/effect/src/unstable/eventlog/EventLog.ts:834` |
| 32 | `layer` | `packages/effect/src/unstable/eventlog/EventLogServerUnencrypted.ts:658` |
| 33 | `layerNoRpcServer` | `packages/effect/src/unstable/eventlog/EventLogServerUnencrypted.ts:680` |
| 34 | `make` | `packages/effect/src/unstable/persistence/PersistedCache.ts:40` |
| 35 | `fromExitWithPrevious` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:137` |
| 36 | `success` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:198` |
| 37 | `failure` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:237` |
| 38 | `failureWithPrevious` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:256` |
| 39 | `fail` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:277` |
| 40 | `failWithPrevious` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:286` |
| 41 | `waiting` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:298` |
| 42 | `replacePrevious` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:327` |
| 43 | `flatMap` | `packages/effect/src/unstable/reactivity/AsyncResult.ts:425` |
| 44 | `readable` | `packages/effect/src/unstable/reactivity/Atom.ts:319` |
| 45 | `writable` | `packages/effect/src/unstable/reactivity/Atom.ts:335` |
| 46 | `make` (Effect factory overload) | `packages/effect/src/unstable/reactivity/Atom.ts:361` |
| 47 | `make` (Stream factory overload) | `packages/effect/src/unstable/reactivity/Atom.ts:361` |
| 48 | `make` (Effect value overload) | `packages/effect/src/unstable/reactivity/Atom.ts:361` |
| 49 | `make` (Stream value overload) | `packages/effect/src/unstable/reactivity/Atom.ts:361` |
| 50 | `fnSync` | `packages/effect/src/unstable/reactivity/Atom.ts:951` |
| 51 | `fn` (Effect overload) | `packages/effect/src/unstable/reactivity/Atom.ts:1031` |
| 52 | `fn` (Stream overload) | `packages/effect/src/unstable/reactivity/Atom.ts:1031` |
| 53 | `pull` | `packages/effect/src/unstable/reactivity/Atom.ts:1145` |
| 54 | `searchParam` | `packages/effect/src/unstable/reactivity/Atom.ts:1949` |
| 55 | `getResult` | `packages/effect/src/unstable/reactivity/Atom.ts:2093` |
| 56 | `dehydrate` | `packages/effect/src/unstable/reactivity/Hydration.ts:31` |
| 57 | `hydrate` | `packages/effect/src/unstable/reactivity/Hydration.ts:85` |
| 58 | `process` | `packages/effect/src/unstable/workflow/DurableQueue.ts:159` |
| 59 | `makeWorker` | `packages/effect/src/unstable/workflow/DurableQueue.ts:235` |
| 60 | `worker` | `packages/effect/src/unstable/workflow/DurableQueue.ts:322` |
| 61 | `toRpcGroup` | `packages/effect/src/unstable/workflow/WorkflowProxy.ts:49` |
| 62 | `toHttpApiGroup` | `packages/effect/src/unstable/workflow/WorkflowProxy.ts:133` |
| 63 | `layerHttpApi` | `packages/effect/src/unstable/workflow/WorkflowProxyServer.ts:19` |
| 64 | `layerRpcHandlers` | `packages/effect/src/unstable/workflow/WorkflowProxyServer.ts:83` |
| 65 | `layerLoggerProvider` | `packages/opentelemetry/src/Logger.ts:130` |
| 66 | `registerProducer` | `packages/opentelemetry/src/Metrics.ts:48` |
| 67 | `layer` | `packages/opentelemetry/src/Metrics.ts:104` |
| 68 | `layerTracerProvider` | `packages/opentelemetry/src/NodeSdk.ts:44` |
| 69 | `layerTracerProvider` | `packages/opentelemetry/src/WebSdk.ts:42` |
| 70 | `layerWebSocket` | `packages/platform-browser/src/BrowserSocket.ts:11` |
| 71 | `fromEventListenerWindow` | `packages/platform-browser/src/BrowserStream.ts:16` |
| 72 | `fromEventListenerDocument` | `packages/platform-browser/src/BrowserStream.ts:35` |
| 73 | `make` | `packages/platform-browser/src/IndexedDbDatabase.ts:243` |
| 74 | `make` | `packages/sql/mssql/src/Parameter.ts:36` |
| 75 | `makeCompiler` | `packages/sql/pglite/src/PgliteClient.ts:335` |
| 76 | `imports` | `packages/tools/openapi-generator/src/HttpApiTransformer.ts:27` |
| 77 | `toImplementation` | `packages/tools/openapi-generator/src/HttpApiTransformer.ts:39` |
| 78 | `spreadElementsInto` | `packages/tools/openapi-generator/src/Utils.ts:46` |
| 79 | `inputKey` | `packages/effect/test/unstable/cli/services/MockTerminal.ts:104` |
| 80 | `logAction` | `packages/effect/test/unstable/cli/services/TestActions.ts:23` |
| 81 | `suite` | `packages/effect/test/unstable/eventlog/SqlEventLogServerUnencryptedStorageTest.ts:47` |
| 82 | `suite` | `packages/effect/test/unstable/persistence/KeyValueStoreTest.ts:6` |
| 83 | `suite` | `packages/effect/test/unstable/persistence/PersistedCacheTest.ts:23` |
| 84 | `suite` | `packages/effect/test/unstable/persistence/PersistedQueueTest.ts:6` |
| 85 | `e2eSuite` | `packages/platform-browser/test/fixtures/rpc-e2e.ts:26` |
| 86 | `e2eSuite` | `packages/platform-node/test/fixtures/rpc-e2e.ts:26` |
| 87 | `runRule` | `packages/tools/oxc/test/utils.ts:43` |

This is the root-project result set. Direct single-file checks can produce additional findings in files that the root project-reference diagnostic run does not visit, such as stable `Array.ts` and `Function.ts` APIs.
