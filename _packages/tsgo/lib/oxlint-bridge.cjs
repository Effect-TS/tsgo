"use strict";

// Preloaded by `effect-tsgo oxlint` and `effect-tsgo vp` into the Node.js and Bun processes they start
// (NODE_OPTIONS=--require, BUN_OPTIONS=--preload=). Both runtimes load N-API addons through process.dlopen, so the
// stock Oxlint binding is mapped there to the Effect-patched one named by EFFECT_TSGO_OXLINT_BINDING. Every other
// addon (vite-plus, rolldown, ...) loads unchanged, which NAPI_RS_NATIVE_LIBRARY_PATH cannot ensure: every napi-rs
// loader in the process reads it.
const path = require("node:path");

const patchedBinding = process.env.EFFECT_TSGO_OXLINT_BINDING;
const stockBinding = /[\\/]@oxlint[\\/]binding-[\w-]+[\\/]oxlint\.[\w-]+\.node$/i;

if (patchedBinding && typeof process.dlopen === "function") {
  const dlopen = process.dlopen;
  // Node.js passes the namespaced form of long Windows paths; keep it for the replacement.
  const replacement = path.toNamespacedPath(patchedBinding);
  // Only the arguments the caller passed are forwarded: an explicit undefined flags argument is not the default.
  process.dlopen = (module, filename, ...rest) =>
    dlopen(module, typeof filename === "string" && stockBinding.test(filename) ? replacement : filename, ...rest);
}
