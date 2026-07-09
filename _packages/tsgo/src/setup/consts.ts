import * as pkgJson from "../../package.json"

export const LSP_PACKAGE_NAME = pkgJson.name
export const LSP_PLUGIN_NAME = "@effect/language-service"
export const TYPESCRIPT_PACKAGE_NAME = "typescript"
export const PATCH_COMMAND = "effect-tsgo patch"
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

/** The `typescript` >= 7 backend. */
export const typescriptBackend: NativeBackend = {
  packageName: TYPESCRIPT_PACKAGE_NAME,
  platformPackagePrefix: "@typescript/typescript",
  binaryName: "tsc",
  versionCheck: (pkg) => isNativeTypescriptVersion(pkg.version ?? "0")
}

/**
 * Resolve the VS Code TypeScript 7 tsdk folder.
 */
export const nativeBackendTsdkPath = (packageName: string): string =>
  "node_modules/" + packageName
