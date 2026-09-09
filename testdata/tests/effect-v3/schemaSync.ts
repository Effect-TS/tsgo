// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics schemaSync:warning
import { Effect, Schema } from "effect"
import * as S from "effect/Schema"
import { decodeUnknownSync as decode } from "effect/Schema"
import * as Parser from "effect/ParseResult"

const Person = Schema.Struct({ name: Schema.String })
const input = { name: "Ada" }

// All four synchronous methods are flagged outside Effect contexts.
export const decodeSyncValue = Schema.decodeSync(Person)(input)
export const decodeUnknownSyncValue = Schema.decodeUnknownSync(Person)(input)
export const encodeSyncValue = Schema.encodeSync(Person)(input)
export const encodeUnknownSyncValue = Schema.encodeUnknownSync(Person)(input)

// Namespace aliases, named import aliases, and decoder factories are covered.
export const namespaceAlias = S.decodeSync(Person)(input)
export const namedAlias = decode(Person)(input)
const decodeAlias = Schema.decodeUnknownSync
export const constantAlias = decodeAlias(Person)(input)
export const elementAccess = Schema["encodeSync"](Person)(input)
export const decoder = Schema.decodeSync(Person)
export const regularFunction = () => Schema.encodeSync(Person)(input)
export const generator = Effect.gen(function*() {
  return Schema.decodeSync(Person)(input)
})

// The underlying parser APIs are covered too.
export const parserdecodeSync = Parser.decodeSync(Person)(input)
export const parserdecodeUnknownSync = Parser.decodeUnknownSync(Person)(input)
export const parserencodeSync = Parser.encodeSync(Person)(input)
export const parserencodeUnknownSync = Parser.encodeUnknownSync(Person)(input)

// Effect and other non-throwing adapters remain allowed.
export const alloweddecode = Schema.decode(Person)(input)
export const alloweddecodeOption = Schema.decodeOption(Person)(input)
export const alloweddecodeEither = Schema.decodeEither(Person)(input)
export const alloweddecodeUnknown = Schema.decodeUnknown(Person)(input)
export const alloweddecodeUnknownOption = Schema.decodeUnknownOption(Person)(input)
export const alloweddecodeUnknownEither = Schema.decodeUnknownEither(Person)(input)
export const allowedencode = Schema.encode(Person)(input)
export const allowedencodeOption = Schema.encodeOption(Person)(input)
export const allowedencodeEither = Schema.encodeEither(Person)(input)
export const allowedencodeUnknown = Schema.encodeUnknown(Person)(input)
export const allowedencodeUnknownOption = Schema.encodeUnknownOption(Person)(input)
export const allowedencodeUnknownEither = Schema.encodeUnknownEither(Person)(input)

// Unrelated APIs with the same names are not Schema methods.
const unrelated = {
  decodeSync: (value: string) => value,
  decodeUnknownSync: (value: unknown) => value,
  encodeSync: (value: string) => value,
  encodeUnknownSync: (value: unknown) => value
}
export const otherDecode = unrelated.decodeSync("hello")
export const otherDecodeUnknown = unrelated.decodeUnknownSync(input)
export const otherEncode = unrelated.encodeSync("hello")
export const otherEncodeUnknown = unrelated.encodeUnknownSync(input)
