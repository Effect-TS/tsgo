import * as Array from "effect/Array"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import type * as PlatformError from "effect/PlatformError"
import type * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import { FileReadError, TsConfigNotFoundError } from "./errors.js"
import type { FileInput } from "./types.js"

const findTsConfigFiles = (
  currentDir: string
): Effect.Effect<ReadonlyArray<string>, PlatformError.PlatformError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const path = yield* Path.Path

    const files = yield* fs.readDirectory(currentDir)
    const tsconfigFiles = Array.filter(files, (file) => {
      const fileName = file.toLowerCase()
      return fileName.startsWith("tsconfig") && (fileName.endsWith(".json") || fileName.endsWith(".jsonc"))
    }).map((file) => path.join(currentDir, file))

    return tsconfigFiles
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
      selectedTsconfigPath = yield* Prompt.text({
        message: "Enter path to your tsconfig.json file"
      })
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
        selectedTsconfigPath = yield* Prompt.text({
          message: "Enter path to your tsconfig.json file"
        })
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
