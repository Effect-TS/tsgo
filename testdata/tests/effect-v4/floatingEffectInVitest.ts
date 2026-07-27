import { afterAll, afterEach, beforeAll, beforeEach, describe, it, suite, test } from "@effect/vitest"
import { Effect } from "effect"
import { it as importedIt } from "vitest"
import * as Vitest from "vitest"

declare const arbitrary: any

const effectCallback = () => Effect.void

it("direct", () => Effect.void)
it.only("only", () => Effect.void)
it.skip("skip", () => Effect.void)
it.each([1, 2])("each %s", () => Effect.void)
it.for([1, 2])("for %s", () => Effect.void)
it.skipIf(false)("skipIf", () => Effect.void)
it.runIf(true)("runIf", () => Effect.void)
it("options first", { retry: 1 }, () => Effect.void)
it.prop("property", [arbitrary], () => Effect.void)
it("named callback", effectCallback)
it("block callback", () => {
  return Effect.void
})
it("conditional return", (ctx) => ctx.task.name === "x" ? Effect.void : undefined)
it("Promise of Effect", () => Promise.resolve(Effect.void))

test("test alias", () => Effect.void)
importedIt("named import", () => Effect.void)
Vitest.it("namespace import", () => Effect.void)
Vitest.test.only("namespace modifier", () => Effect.void)

beforeAll(() => Effect.void)
beforeEach(() => Effect.void)
afterAll(() => Effect.void)
afterEach(() => Effect.void)
it.beforeEach(() => Effect.void)

it.effect("effect aware", () => Effect.void)
it.effect.only("effect aware only", () => Effect.void)
it.effect.each([1, 2])("effect aware each %s", () => Effect.void)
it.effect.prop("effect aware property", [arbitrary], () => Effect.void)
it.live("live aware", () => Effect.void)

it("runPromise", () => Effect.runPromise(Effect.void))
it("ordinary Promise", () => Promise.resolve(1))
it("void", () => undefined)

describe("suite omitted", () => Effect.void)
suite("suite alias omitted", () => Effect.void)
it.describe("attached suite omitted", () => Effect.void)
