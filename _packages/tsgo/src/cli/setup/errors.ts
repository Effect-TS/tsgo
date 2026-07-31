import * as Data from "effect/Data"

export class PackageJsonNotFoundError extends Data.TaggedError("PackageJsonNotFoundError")<{
  readonly path: string
}> {
  get message() {
    return `No package.json found at ${this.path}. Please run this command in the root of your project.`
  }
}

export class TsConfigNotFoundError extends Data.TaggedError("TsConfigNotFoundError")<{
  readonly path: string
}> {
  get message() {
    return `No tsconfig file found at ${this.path}.`
  }
}

export class FileReadError extends Data.TaggedError("FileReadError")<{
  readonly path: string
  readonly cause: unknown
}> {
  get message() {
    return `Unable to read file at ${this.path}`
  }
}
