# Oxlint patches

Store Effect-owned patches against the Oxlint revision recorded in
`_packages/tsgo/upstream.json` here. `repoctl submodules setup --component oxlint --version <version>`
applies `*.patch` files in bytewise filename order.

`001-effect-plugin.patch` registers the built-in Effect tsgo plugin and preserves
qualified `effecttsgo/*` rule identities across the tsgolint protocol.
