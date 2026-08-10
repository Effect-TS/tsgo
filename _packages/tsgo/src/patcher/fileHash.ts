import * as Crypto from "effect/Crypto"
import * as Effect from "effect/Effect"
import * as Encoding from "effect/Encoding"
import * as FileSystem from "effect/FileSystem"

const textEncoder = new TextEncoder()

export const hashBytes = (contents: string | Uint8Array) => Effect.gen(function*() {
  const crypto = yield* Crypto.Crypto
  const digest = yield* crypto.digest("SHA-256", typeof contents === "string" ? textEncoder.encode(contents) : contents)
  return Encoding.encodeHex(digest)
})

export const hashFile = (filePath: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  return yield* hashBytes(yield* fs.readFile(filePath))
})
