# Oxlint patches

Store Effect-owned patches against the Oxlint revision recorded in
`_packages/tsgo/upstream.json` here. `repoctl submodules setup --component oxlint --version <version>`
applies `*.patch` files in bytewise filename order.

`001-effect-plugin.patch` registers the built-in Effect tsgo plugin and preserves
qualified `effecttsgo/*` rule identities across the tsgolint protocol.

`002-effect-tsgolint-path.patch` makes the binding run the tsgolint named by
`EFFECT_TSGO_TSGOLINT_PATH` before `OXLINT_TSGOLINT_PATH`: the `effect-tsgo oxlint`
and `effect-tsgo vp` proxies set it, and vite-plus overwrites `OXLINT_TSGOLINT_PATH`
with the stock tsgolint for the Oxlint process it starts. With `EFFECT_TSGO_TSGOLINT_MODE=tsrs`
(set by `effect-tsgo --tsrs-lint oxlint|vp`), the binary of `EFFECT_TSGO_TSGOLINT_PATH` is the
packaged tsrs, which the binding runs as `tsrs --lint headless ...`; the Go profiling variables
(`OXLINT_TSGOLINT_TRACE`, `_CPU`, `_HEAP`, `_ALLOCS`) are rejected in that mode.
