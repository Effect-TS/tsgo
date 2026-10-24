import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { Command } from "effect/cli"
import { describe, expect, it } from "vitest"
import { ensureIntegrationSelected, integrationFlags } from "../src/cli/integrationFlags.js"

const parseIntegrationFlags = async (args: ReadonlyArray<string>) => {
  let result: { readonly typescript: boolean; readonly oxlint: boolean; readonly tsrs: boolean } | undefined
  const command = Command.make("patch", integrationFlags).pipe(
    Command.withHandler((flags) => Effect.sync(() => result = flags))
  )
  await Effect.runPromise(
    Command.runWith(command, { version: "test" })(args).pipe(Effect.provide(NodeServices.layer))
  )
  return result
}

describe("integration flags", () => {
  it("patches only TypeScript by default", async () => {
    await expect(parseIntegrationFlags([])).resolves.toEqual({ typescript: true, oxlint: false, tsrs: false })
  })

  it("adds Oxlint without disabling TypeScript", async () => {
    await expect(parseIntegrationFlags(["--oxlint"]))
      .resolves.toEqual({ typescript: true, oxlint: true, tsrs: false })
  })

  it("supports selecting only Oxlint", async () => {
    await expect(parseIntegrationFlags(["--no-typescript", "--oxlint"]))
      .resolves.toEqual({ typescript: false, oxlint: true, tsrs: false })
  })

  it("adds the opt-in tsrs integration, alone or next to TypeScript", async () => {
    await expect(parseIntegrationFlags(["--tsrs"]))
      .resolves.toEqual({ typescript: true, oxlint: false, tsrs: true })
    await expect(parseIntegrationFlags(["--no-typescript", "--tsrs"]))
      .resolves.toEqual({ typescript: false, oxlint: false, tsrs: true })
    await Effect.runPromise(ensureIntegrationSelected(false, false, true))
  })

  it("rejects disabling every integration", async () => {
    await expect(Effect.runPromise(ensureIntegrationSelected(false, false)))
      .rejects.toThrow("Select at least one integration")
    await expect(Effect.runPromise(ensureIntegrationSelected(false, false, false)))
      .rejects.toThrow("Select at least one integration")
  })
})
