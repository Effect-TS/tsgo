// PROTOTYPE: generated-shape Oxlint wrappers over shared Effect semantic diagnostics.
import metadataJson from "../../metadata.json" with { type: "json" }
import { abort, finish, register, resultsFor } from "./bridge.ts"

interface RuleMetadata {
  readonly name: string
  readonly description: string
  readonly fixable: boolean
}

interface OxlintFixer {
  replaceTextRange(range: readonly [number, number], text: string): unknown
}

interface OxlintContext {
  readonly cwd: string
  readonly filename: string
  readonly sourceCode: { readonly text: string }
  readonly settings: Readonly<Record<string, unknown>>
  report(diagnostic: {
    readonly message: string
    readonly node: { readonly range: readonly [number, number] }
    readonly suggest?: ReadonlyArray<{
      readonly desc: string
      readonly fix: (fixer: OxlintFixer) => ReadonlyArray<unknown>
    }>
  }): void
}

const metadata = metadataJson as {
  readonly rules: ReadonlyArray<RuleMetadata>
}

const rules = Object.fromEntries(metadata.rules.map((rule) => [rule.name, {
  meta: {
    type: "problem",
    docs: { description: rule.description },
    hasSuggestions: rule.fixable,
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
              suggest: diagnostic.actions?.map((action) => ({
                desc: action.title,
                fix: (fixer: OxlintFixer) => action.edits.map((edit) =>
                  fixer.replaceTextRange([edit.start, edit.end], edit.newText)),
              })),
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
