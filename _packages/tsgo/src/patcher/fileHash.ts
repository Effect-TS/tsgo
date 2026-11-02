import * as Digest from "effect/crypto/Digest"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"

const textEncoder = new TextEncoder()

export const hashBytes = (contents: string | Uint8Array) => Effect.gen(function*() {
  const digest = yield* Digest.Digest
  return yield* digest.digestHex(Digest.sha256, typeof contents === "string" ? textEncoder.encode(contents) : contents)
})

export const hashFile = (filePath: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  return yield* hashBytes(yield* fs.readFile(filePath))
})
