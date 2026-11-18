import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import { Flag } from "effect/cli"
import type { Component } from "../patcher/index.js"

export class IntegrationSelectionError extends Data.TaggedError("IntegrationSelectionError")<{
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

export const integrationFlags = {
  typescript: Flag.Boolean("typescript").pipe(
    Flag.withDefault(true),
    Flag.withDescription("Include the native TypeScript-Go integration")
  ),
  oxlint: Flag.Boolean("oxlint").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Include the experimental Oxlint integration")
  ),
  tsrs: Flag.Boolean("tsrs").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Include the experimental tsrs integration (the installed tsrs package's binary)")
  )
}

export const selectedComponents = (typescript: boolean, oxlint: boolean, tsrs: boolean): ReadonlySet<Component> => {
  const components = new Set<Component>()
  if (typescript) components.add("typescript")
  if (oxlint) components.add("oxlint")
  if (tsrs) components.add("tsrs")
  return components
}

export const ensureIntegrationSelected = (typescript: boolean, oxlint: boolean, tsrs = false) =>
  typescript || oxlint || tsrs
    ? Effect.void
    : Effect.fail(new IntegrationSelectionError({ reason: "Select at least one integration to manage." }))
