---
"@effect/tsgo": patch
---

Fix `repoctl upstream update` failing when a TypeScript commit is reachable from both supported provider repositories.

Previously, `resolveTypeScriptProvider` and `readRemoteTypeScriptGitlink` in `_tools/repoctl/src/upstream.ts` errored out with `TypeScript commit <sha> matched 2 supported repositories` whenever a single revision existed in both `microsoft/TypeScript` (the canonical upstream) and the legacy `microsoft/typescript-go` fork. This blocked the daily "Update Upstreams" workflow for revisions shared between the two.

Both functions now prefer `microsoft/TypeScript` when it matches and fall back to `microsoft/typescript-go` otherwise. A "no supported repository matched" error is still raised when neither matches.
