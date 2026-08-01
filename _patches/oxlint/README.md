# Oxlint patches

Store Effect-owned patches against the Oxlint revision recorded in
`_packages/tsgo/upstream.json` here. `repoctl submodules setup --profile oxlint`
applies `*.patch` files in bytewise filename order.

`001-effect-plugin.patch` registers the built-in Effect plugin and preserves
qualified `effect/*` rule identities across the tsgolint protocol.
