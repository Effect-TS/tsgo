import { createHash } from "node:crypto"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"

export const hashBytes = (contents: string | Uint8Array): string =>
  createHash("sha256").update(contents).digest("hex")

export const hashFile = (filePath: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  return hashBytes(yield* fs.readFile(filePath))
})
