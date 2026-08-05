export type Component = "typescript" | "oxlint" | "oxlint-tsgolint"

export interface DiscoveredBinary {
  readonly component: Component
  readonly packageName: string
  readonly packageVersion: string
  readonly binaryPath: string
}

export type FileSystemOperation = RenameOperation | CopyOperation | ChmodOperation | RemoveOperation

export interface RenameOperation {
  readonly _tag: "Rename"
  readonly sourcePath: string
  readonly destinationPath: string
}

export interface CopyOperation {
  readonly _tag: "Copy"
  readonly sourcePath: string
  readonly destinationPath: string
}

export interface ChmodOperation {
  readonly _tag: "Chmod"
  readonly path: string
  readonly mode: number
}

export interface RemoveOperation {
  readonly _tag: "Remove"
  readonly path: string
}

export interface SkippedTarget {
  readonly target: DiscoveredBinary
  readonly reason: "already-patched" | "no-backup" | "replacement-unavailable"
  readonly message: string
}

export interface PreparedPlan {
  readonly operations: ReadonlyArray<FileSystemOperation>
  readonly cleanup: ReadonlyArray<RemoveOperation>
  readonly skipped: ReadonlyArray<SkippedTarget>
}
