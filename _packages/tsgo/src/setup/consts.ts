import * as pkgJson from "../../package.json"

export const LSP_PACKAGE_NAME = pkgJson.name
export const LSP_PLUGIN_NAME = "@effect/language-service"
export const PATCH_COMMAND = "effect-tsgo patch"
export const DEFAULT_LSP_VERSION = pkgJson.version