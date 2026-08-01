# tsgolint patches

Store Effect-owned patches against the tsgolint revision recorded in
`_packages/tsgo/upstream.json` here. `repoctl submodules setup --profile oxlint`
applies `*.patch` files in bytewise filename order.

`001-effect-rules.patch` registers the generated Effect rule adapters and links
tsgolint to this repository's public Effect runner.
