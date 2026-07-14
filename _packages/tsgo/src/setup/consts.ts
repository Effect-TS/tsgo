import * as pkgJson from "../../package.json"

export const LSP_PACKAGE_NAME = pkgJson.name
export const LSP_PLUGIN_NAME = "@effect/language-service"
export const defaultTypescriptPackageNames = ["typescript", "@typescript/native"] as const
export const PATCH_COMMAND = "effect-tsgo patch"

/**
 * `typescript` package versions >= 7 ship the native Go-ported binary that this
 * tool patches. Older `typescript` releases (<= 6) are the JS compiler and must
 * not be treated as a native backend.
 */
export const isNativeTypescriptVersion = (version: string): boolean => {
  const match = /\d+/.exec(version.trim())
  return match !== null && Number(match[0]) >= 7
}
