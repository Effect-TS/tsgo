import * as pkgJson from "../../package.json"

export const LSP_PACKAGE_NAME = pkgJson.name
export const LSP_PLUGIN_NAME = "@effect/language-service"
export const NATIVE_PREVIEW_PACKAGE_NAME = "@typescript/native-preview"
export const TYPESCRIPT_PACKAGE_NAME = "typescript"
export const PATCH_COMMAND = "effect-tsgo patch"
export const DEFAULT_LSP_VERSION = pkgJson.version
export const DEFAULT_NATIVE_PREVIEW_VERSION = "latest"
export const TSCONFIG_SCHEMA_URL = "https://raw.githubusercontent.com/Effect-TS/tsgo/refs/heads/main/schema.json"

/**
 * `typescript` package versions >= 7 ship the native Go-ported binary that this
 * tool patches. Older `typescript` releases (<= 6) are the JS compiler and must
 * not be treated as a native backend.
 */
export const isNativeTypescriptVersion = (version: string): boolean => {
  const match = /\d+/.exec(version.trim())
  return match !== null && Number(match[0]) >= 7
}

/**
 * Describes a native TypeScript backend that ships the Go-ported binary in a
 * platform-specific sub-package under `lib/<binaryName>`.
 */
export interface NativeBackend {
  readonly packageName: string
  readonly platformPackagePrefix: string
  readonly binaryName: string
  readonly versionCheck?: (pkgJson: { readonly version?: string }) => boolean
}

/** The `@typescript/native-preview` nightly backend (back-compat default). */
export const nativePreviewBackend: NativeBackend = {
  packageName: NATIVE_PREVIEW_PACKAGE_NAME,
  platformPackagePrefix: "@typescript/native-preview",
  binaryName: "tsgo"
}

/** The `typescript` >= 7 backend (stable/RC releases). */
export const typescriptBackend: NativeBackend = {
  packageName: TYPESCRIPT_PACKAGE_NAME,
  platformPackagePrefix: "@typescript/typescript",
  binaryName: "tsc",
  versionCheck: (pkg) => isNativeTypescriptVersion(pkg.version ?? "0")
}

/**
 * Resolve the VS Code `typescript.native-preview.tsdk` folder for a backend.
 * The "TypeScript (Native Preview)" extension reads the native install from
 * this path; for `@typescript/native-preview` it is `node_modules/<pkg>`, and
 * for `typescript` >= 7 it is `node_modules/typescript`.
 */
export const nativeBackendTsdkPath = (packageName: string): string =>
  "node_modules/" + packageName
