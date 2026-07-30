// PROTOTYPE: generated-shape Oxlint wrappers over shared Effect semantic diagnostics.
import { readFileSync } from "node:fs"

import { abort, finish, register, resultsFor } from "./bridge.ts"

interface RuleMetadata {
  readonly name: string
  readonly description: string
}

interface OxlintContext {
  readonly filename: string
  readonly sourceCode: { readonly text: string }
  readonly settings: Readonly<Record<string, unknown>>
  report(diagnostic: { readonly message: string; readonly node: { readonly range: readonly [number, number] } }): void
}

const metadata = JSON.parse(readFileSync(new URL("../../metadata.json", import.meta.url), "utf8")) as {
  readonly rules: ReadonlyArray<RuleMetadata>
}

const rules = Object.fromEntries(metadata.rules.map((rule) => [rule.name, {
  meta: {
    type: "problem",
    docs: { description: rule.description },
  },
  createOnce(context: OxlintContext) {
    return {
      Program() {
        register(rule.name, context)
      },
      "Program:exit"() {
        try {
          for (const diagnostic of resultsFor(rule.name)) {
            context.report({
              message: diagnostic.message,
              node: { range: [diagnostic.start, diagnostic.end] },
            })
          }
        } finally {
          finish()
        }
      },
      after() {
        abort()
      },
    }
  },
}]))

export default {
  meta: { name: "effect" },
  rules,
}
