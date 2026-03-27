import * as pkgJson from "../../package.json"

export const LSP_PACKAGE_NAME = pkgJson.name
export const LSP_PLUGIN_NAME = "@effect/language-service"
export const NATIVE_PREVIEW_PACKAGE_NAME = "@typescript/native-preview"
export const PATCH_COMMAND = "effect-tsgo patch"
export const DEFAULT_LSP_VERSION = pkgJson.version
export const DEFAULT_NATIVE_PREVIEW_VERSION = "latest"
export const TSCONFIG_SCHEMA_URL = "https://raw.githubusercontent.com/Effect-TS/tsgo/refs/heads/main/schema.json"
