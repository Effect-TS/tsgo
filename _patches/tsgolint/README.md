# tsgolint patches

Store Effect-owned patches against the tsgolint revision recorded in
`_packages/tsgo/upstream.json` here. `repoctl submodules setup --component oxlint-tsgolint --version <version>`
applies `*.patch` files in bytewise filename order.

`001-effect-rules.patch` registers the generated Effect rule adapters and links
tsgolint to this repository's public Effect runner.
