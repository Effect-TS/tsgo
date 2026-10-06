# Integration sources

Additive compiler sources that belong to this repository rather than the
upstream patch stack live here, one directory per compiler checkout:
`_integrations/<checkoutDir>/` mirrors the checkout root, so
`_integrations/typescript/tsc/internal/checker/integration.go` is installed at
`typescript/tsc/internal/checker/integration.go`.

`repoctl submodules setup` copies every `_integrations/<checkoutDir>` tree into
the selected compiler checkout through `installIntegrationSources`; the copy is
idempotent and runs before the diagnostics and shim generators load the
compiler packages. Integration files are untracked in the checkout, so a
`git reset --hard`/`git clean -fdx` from setup removes them and the next setup
restores them from here.

Provider-specific checker accessors belong in both
`_integrations/typescript/tsc/internal/checker/integration.go` and
`_integrations/typescript-go/internal/checker/integration.go` when Effect code
uses them through the canonical TypeScript shim. Keep their behavior aligned;
the legacy checkout may need small compatibility accessors where its compiler
types do not expose the equivalent API.

Use an integration file for additive provider-owned code that cannot be shimmed
from a separate overlay package (for example accessors on a compiler type that
the Effect code reaches through a shim type alias). Prefer the matching
`_patches/<provider>` patch stack when the change edits upstream files rather
than adding a new one.

The generated-shim cache key in `_tools/gen_shims` hashes `_integrations`, and
the checkout copy is part of the compiler source digest, so editing an
integration source invalidates cached shims instead of reusing stale output.
The Nix flake copies `_integrations/typescript` into its patched source tree as
well. The GitHub `setup-repo` action includes `_integrations/**` in its Go and
generated-shim cache keys.
