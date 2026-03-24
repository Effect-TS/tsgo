import * as Array from "effect/Array"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import type * as PlatformError from "effect/PlatformError"
import type * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import { FileReadError, TsConfigNotFoundError } from "./errors.js"
import type { FileInput } from "./types.js"

const isTsConfigFile = (file: string) => {
  const fileName = file.toLowerCase()
  return fileName.endsWith(".json") || fileName.endsWith(".jsonc")
}

const findTsConfigFiles = (
  currentDir: string
): Effect.Effect<ReadonlyArray<string>, PlatformError.PlatformError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const path = yield* Path.Path

    const files = yield* fs.readDirectory(currentDir)
    const tsconfigFiles = Array.filter(files, isTsConfigFile).map((file) => path.join(currentDir, file))

    return tsconfigFiles
  })

const promptForTsConfigPath = (currentDir: string) =>
  Prompt.file({
    type: "file",
    message: "Select tsconfig to configure",
    startingPath: currentDir,
    filter: (file) => file === ".." || !file.includes(".") || isTsConfigFile(file)
  })

export const selectTsConfigFile = (
  currentDir: string
): Effect.Effect<
  FileInput,
  PlatformError.PlatformError | Terminal.QuitError | TsConfigNotFoundError | FileReadError,
  Prompt.Environment
> =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const path = yield* Path.Path

    const tsconfigFiles = yield* findTsConfigFiles(currentDir)

    let selectedTsconfigPath: string

    if (tsconfigFiles.length === 0) {
      selectedTsconfigPath = yield* promptForTsConfigPath(currentDir)
    } else {
      const choices = [
        ...tsconfigFiles.map((file) => ({
          title: file,
          value: file
        })),
        {
          title: "Enter path manually",
          value: "__manual__"
        }
      ]

      const selected = yield* Prompt.select({
        message: "Select tsconfig to configure",
        choices
      })

      if (selected === "__manual__") {
        selectedTsconfigPath = yield* promptForTsConfigPath(currentDir)
      } else {
        selectedTsconfigPath = selected
      }
    }

    selectedTsconfigPath = path.resolve(selectedTsconfigPath)

    const tsconfigExists = yield* fs.exists(selectedTsconfigPath)
    if (!tsconfigExists) {
      return yield* new TsConfigNotFoundError({ path: selectedTsconfigPath })
    }

    const tsconfigText = yield* fs.readFileString(selectedTsconfigPath).pipe(
      Effect.mapError((cause) => new FileReadError({ path: selectedTsconfigPath, cause }))
    )

    return {
      fileName: selectedTsconfigPath,
      text: tsconfigText
    }
  })
