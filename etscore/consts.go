package etscore

// EffectPluginName is the name of the Effect language service plugin.
// This is used to identify the plugin in the tsconfig.json plugins array.
const EffectPluginName = "@effect/language-service"

// EffectPluginVersion is appended to the TypeScript version in tsbuildinfo files.
// Bump this whenever diagnostic behavior changes to invalidate stale incremental state.
const EffectPluginVersion = "effect.1"
