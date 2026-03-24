import { describe, it, expect } from "vitest"
import * as Option from "effect/Option"
import { computeChanges } from "../../src/setup/changes.js"
import { assess } from "../../src/setup/assessment.js"
import type { Assessment } from "../../src/setup/types.js"

/**
 * Helper to create an Assessment.Input and run assess() + computeChanges()
 */
function runComputeChanges(opts: {
  packageJsonText?: string
  tsconfigText?: string
  vscodeSettingsText?: string | null
  editors?: ReadonlyArray<"vscode" | "nvim" | "emacs">
  lspVersion?: { dependencyType: "dependencies" | "devDependencies"; version: string } | null
  prepareScript?: boolean
  vscodeTargetSettings?: Record<string, unknown> | null
  diagnosticSeverities?: Record<string, "off" | "suggestion" | "message" | "warning" | "error"> | null
}) {
  const packageJsonText = opts.packageJsonText ?? JSON.stringify({
    name: "test-project",
    version: "1.0.0",
    devDependencies: {}
  }, null, 2)

  const tsconfigText = opts.tsconfigText ?? JSON.stringify({
    compilerOptions: {
      target: "ES2022",
      module: "ESNext",
      moduleResolution: "Bundler"
    }
  }, null, 2)

  const input: Assessment.Input = {
    packageJson: { fileName: "/test/package.json", text: packageJsonText },
    tsconfig: { fileName: "/test/tsconfig.json", text: tsconfigText },
    vscodeSettings: opts.vscodeSettingsText != null
      ? Option.some({ fileName: "/test/.vscode/settings.json", text: opts.vscodeSettingsText })
      : Option.none()
  }

  const assessment = assess(input)

  const lspVersion = opts.lspVersion !== undefined
    ? (opts.lspVersion === null ? Option.none() : Option.some(opts.lspVersion))
    : Option.some({ dependencyType: "devDependencies" as const, version: "0.0.4" })

  const vscodeTargetSettings = opts.vscodeTargetSettings !== undefined
    ? (opts.vscodeTargetSettings === null ? Option.none() : Option.some({ settings: opts.vscodeTargetSettings }))
    : Option.some({ settings: { "typescript.tsserver.experimental.enableProjectDiagnostics": true } })

  const target = {
    packageJson: {
      lspVersion,
      prepareScript: opts.prepareScript ?? true
    },
    tsconfig: {
      diagnosticSeverities: opts.diagnosticSeverities === undefined
        ? Option.none()
        : opts.diagnosticSeverities === null
        ? Option.none()
        : Option.some(opts.diagnosticSeverities)
    },
    vscodeSettings: vscodeTargetSettings,
    editors: opts.editors ?? ["vscode"]
  }

  return computeChanges(assessment, target)
}

describe("computeChanges", () => {
  it("should not throw for Astro-style configs that already include the Effect plugin", () => {
    const packageJsonText = JSON.stringify({
      scripts: {
        prepare: "effect-language-service patch",
        dev: "astro dev"
      },
      dependencies: {},
      devDependencies: {
        "@effect/language-service": "^0.80.0",
        typescript: "^5.9.3"
      }
    }, null, 2)

    const tsconfigText = JSON.stringify({
      extends: "astro/tsconfigs/strictest",
      include: [".astro/types.d.ts", "**/*"],
      exclude: ["dist"],
      compilerOptions: {
        paths: {
          "@/*": ["./src/*"]
        },
        jsx: "react-jsx",
        jsxImportSource: "react",
        skipLibCheck: true,
        plugins: [
          {
            name: "@effect/language-service",
            namespaceImportPackages: ["effect", "@effect/*"]
          }
        ]
      }
    }, null, 2)

    expect(() =>
      runComputeChanges({
        packageJsonText,
        tsconfigText
      })
    ).not.toThrow()
  })

  it("should not throw when only the package.json matches the Astro install shape", () => {
    const packageJsonText = JSON.stringify({
      scripts: {
        prepare: "effect-language-service patch",
        dev: "astro dev"
      },
      dependencies: {},
      devDependencies: {
        "@effect/language-service": "^0.80.0",
        typescript: "^5.9.3"
      }
    }, null, 2)

    expect(() =>
      runComputeChanges({
        packageJsonText,
        prepareScript: false
      })
    ).not.toThrow()
  })

  it("should not throw when updating an existing prepare script from the legacy command", () => {
    const packageJsonText = JSON.stringify({
      scripts: {
        prepare: "effect-language-service patch",
        dev: "astro dev"
      },
      dependencies: {},
      devDependencies: {
        typescript: "^5.9.3"
      }
    }, null, 2)

    expect(() =>
      runComputeChanges({
        packageJsonText,
        prepareScript: true
      })
    ).not.toThrow()
  })

  it("should not throw when only the tsconfig matches the Astro plugin shape", () => {
    const tsconfigText = JSON.stringify({
      extends: "astro/tsconfigs/strictest",
      include: [".astro/types.d.ts", "**/*"],
      exclude: ["dist"],
      compilerOptions: {
        paths: {
          "@/*": ["./src/*"]
        },
        jsx: "react-jsx",
        jsxImportSource: "react",
        skipLibCheck: true,
        plugins: [
          {
            name: "@effect/language-service",
            namespaceImportPackages: ["effect", "@effect/*"]
          }
        ]
      }
    }, null, 2)

    expect(() =>
      runComputeChanges({
        tsconfigText
      })
    ).not.toThrow()
  })

  describe("isNewFile marker", () => {
    it("should set isNewFile to false for package.json modification code actions", () => {
      const result = runComputeChanges({})

      const pkgActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("package.json"))
      )
      expect(pkgActions.length).toBeGreaterThan(0)

      for (const action of pkgActions) {
        for (const change of action.changes) {
          expect(change.isNewFile).toBe(false)
        }
      }
    })

    it("should set isNewFile to false for tsconfig.json modification code actions", () => {
      const result = runComputeChanges({})

      const tsconfigActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigActions.length).toBeGreaterThan(0)

      for (const action of tsconfigActions) {
        for (const change of action.changes) {
          expect(change.isNewFile).toBe(false)
        }
      }
    })

    it("should set isNewFile to false for existing vscode settings modification code actions", () => {
      const result = runComputeChanges({
        vscodeSettingsText: JSON.stringify({}, null, 2),
        vscodeTargetSettings: {
          "typescript.tsserver.experimental.enableProjectDiagnostics": true
        }
      })

      const vscodeActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("settings.json"))
      )
      expect(vscodeActions.length).toBeGreaterThan(0)

      for (const action of vscodeActions) {
        for (const change of action.changes) {
          expect(change.isNewFile).toBe(false)
        }
      }
    })
  })

  describe("new-file code action for .vscode/settings.json", () => {
    it("should emit isNewFile: true when vscodeSettings is None and target requires vscode", () => {
      const result = runComputeChanges({
        vscodeSettingsText: null,
        editors: ["vscode"],
        vscodeTargetSettings: {
          "typescript.tsserver.experimental.enableProjectDiagnostics": true
        }
      })

      const vscodeActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("settings.json"))
      )
      expect(vscodeActions).toHaveLength(1)

      const action = vscodeActions[0]
      expect(action.description).toBe("Create .vscode/settings.json")
      expect(action.changes).toHaveLength(1)

      const fileChange = action.changes[0]
      expect(fileChange.isNewFile).toBe(true)
      expect(fileChange.fileName).toBe("/test/.vscode/settings.json")
    })

    it("should include full JSON content as the text change newText", () => {
      const targetSettings = {
        "typescript.tsserver.experimental.enableProjectDiagnostics": true
      }

      const result = runComputeChanges({
        vscodeSettingsText: null,
        editors: ["vscode"],
        vscodeTargetSettings: targetSettings
      })

      const vscodeAction = result.codeActions.find((a) =>
        a.changes.some((c) => c.fileName.includes("settings.json"))
      )!

      const fileChange = vscodeAction.changes[0]
      expect(fileChange.textChanges).toHaveLength(1)

      const textChange = fileChange.textChanges[0]
      expect(textChange.span).toEqual({ start: 0, length: 0 })

      const expectedContent = JSON.stringify(targetSettings, null, 2) + "\n"
      expect(textChange.newText).toBe(expectedContent)
    })

    it("should not emit new-file action when vscode is not in editors list", () => {
      const result = runComputeChanges({
        vscodeSettingsText: null,
        editors: ["nvim"],
        vscodeTargetSettings: {
          "typescript.tsserver.experimental.enableProjectDiagnostics": true
        }
      })

      const vscodeActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("settings.json"))
      )
      expect(vscodeActions).toHaveLength(0)
    })

    it("should not emit new-file action when lspVersion is None", () => {
      const result = runComputeChanges({
        vscodeSettingsText: null,
        editors: ["vscode"],
        lspVersion: null,
        vscodeTargetSettings: {
          "typescript.tsserver.experimental.enableProjectDiagnostics": true
        }
      })

      const vscodeActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("settings.json"))
      )
      expect(vscodeActions).toHaveLength(0)
    })

    it("should emit new-file action with multiple settings", () => {
      const targetSettings = {
        "typescript.tsserver.experimental.enableProjectDiagnostics": true,
        "editor.defaultFormatter": "vscode.typescript-language-features"
      }

      const result = runComputeChanges({
        vscodeSettingsText: null,
        editors: ["vscode"],
        vscodeTargetSettings: targetSettings
      })

      const vscodeAction = result.codeActions.find((a) =>
        a.changes.some((c) => c.fileName.includes("settings.json"))
      )!

      const expectedContent = JSON.stringify(targetSettings, null, 2) + "\n"
      expect(vscodeAction.changes[0].textChanges[0].newText).toBe(expectedContent)
    })
  })

  describe("post-apply messages", () => {
    it("should include patch message when installing", () => {
      const result = runComputeChanges({})

      expect(result.messages).toContain(
        "Run `effect-tsgo patch` to complete the installation."
      )
    })

    it("should include unpatch message when uninstalling a previously installed LSP", () => {
      const packageJsonText = JSON.stringify({
        name: "test-project",
        version: "1.0.0",
        devDependencies: {
          "@effect/tsgo": "0.0.4"
        }
      }, null, 2)

      const result = runComputeChanges({
        packageJsonText,
        lspVersion: null,
        editors: [],
        vscodeTargetSettings: null
      })

      expect(result.messages).toContain(
        "Run `effect-tsgo unpatch` to restore the original TypeScript-Go binary."
      )
    })

    it("should not include unpatch message when LSP was not previously installed", () => {
      const result = runComputeChanges({
        lspVersion: null,
        editors: [],
        vscodeTargetSettings: null
      })

      expect(result.messages).not.toContain(
        "Run `effect-tsgo unpatch` to restore the original TypeScript-Go binary."
      )
    })
  })

  describe("tsconfig with missing compilerOptions", () => {
    it("should add compilerOptions with plugin when tsconfig has no compilerOptions", () => {
      const tsconfigText = JSON.stringify({
        extends: "./base-tsconfig.json"
      }, null, 2)

      const result = runComputeChanges({ tsconfigText })

      const tsconfigAction = result.codeActions.find((a) =>
        a.changes.some((c) => c.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigAction).toBeDefined()
      expect(tsconfigAction!.description).toContain("compilerOptions")
      expect(tsconfigAction!.description).toContain("@effect/language-service")

      const rendered = tsconfigAction!.changes.flatMap((c) => c.textChanges.map((tc) => tc.newText)).join("")
      expect(rendered).toContain("compilerOptions")
      expect(rendered).toContain("plugins")
      expect(rendered).toContain("@effect/language-service")
    })

    it("should add $schema when creating compilerOptions on a tsconfig without it", () => {
      const tsconfigText = JSON.stringify({
        extends: "./base-tsconfig.json"
      }, null, 2)

      const result = runComputeChanges({ tsconfigText })

      const tsconfigAction = result.codeActions.find((a) =>
        a.changes.some((c) => c.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigAction).toBeDefined()
      expect(tsconfigAction!.description).toContain("$schema")

      const rendered = tsconfigAction!.changes.flatMap((c) => c.textChanges.map((tc) => tc.newText)).join("")
      expect(rendered).toContain("$schema")
      expect(rendered).toContain("raw.githubusercontent.com")
    })

    it("should update $schema when creating compilerOptions on a tsconfig with wrong $schema", () => {
      const tsconfigText = JSON.stringify({
        $schema: "https://example.com/wrong-schema.json",
        extends: "./base-tsconfig.json"
      }, null, 2)

      const result = runComputeChanges({ tsconfigText })

      const tsconfigAction = result.codeActions.find((a) =>
        a.changes.some((c) => c.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigAction).toBeDefined()
      expect(tsconfigAction!.description).toContain("Update $schema")

      const rendered = tsconfigAction!.changes.flatMap((c) => c.textChanges.map((tc) => tc.newText)).join("")
      expect(rendered).toContain("raw.githubusercontent.com")
      expect(rendered).not.toContain("example.com")
    })

    it("should include diagnosticSeverity in plugin when configured with missing compilerOptions", () => {
      const tsconfigText = JSON.stringify({
        extends: "./base-tsconfig.json"
      }, null, 2)

      const result = runComputeChanges({
        tsconfigText,
        diagnosticSeverities: {
          floatingEffect: "warning",
          missingEffectError: "off"
        }
      })

      const tsconfigAction = result.codeActions.find((a) =>
        a.changes.some((c) => c.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigAction).toBeDefined()

      const rendered = tsconfigAction!.changes.flatMap((c) => c.textChanges.map((tc) => tc.newText)).join("")
      expect(rendered).toContain("diagnosticSeverity")
      expect(rendered).toContain("floatingEffect")
      expect(rendered).toContain("missingEffectError")
    })

    it("should produce no tsconfig code actions when lspVersion is null and compilerOptions missing", () => {
      const tsconfigText = JSON.stringify({
        extends: "./base-tsconfig.json"
      }, null, 2)

      const result = runComputeChanges({
        tsconfigText,
        lspVersion: null,
        editors: [],
        vscodeTargetSettings: null
      })

      const tsconfigActions = result.codeActions.filter((a) =>
        a.changes.some((c) => c.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigActions).toHaveLength(0)
    })
  })

  describe("tsconfig diagnosticSeverity", () => {
    it("should add diagnosticSeverity to the Effect plugin when configured", () => {
      const result = runComputeChanges({
        diagnosticSeverities: {
          floatingEffect: "warning",
          missingEffectError: "off"
        }
      })

      const tsconfigAction = result.codeActions.find((action) =>
        action.changes.some((change) => change.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigAction).toBeDefined()
      const rendered = tsconfigAction!.changes.flatMap((change) => change.textChanges.map((textChange) => textChange.newText)).join("\n")
      expect(rendered).toContain("diagnosticSeverity")
      expect(rendered).toContain("floatingEffect")
      expect(rendered).toContain("missingEffectError")
    })

    it("should remove diagnosticSeverity when target uses defaults", () => {
      const result = runComputeChanges({
        tsconfigText: JSON.stringify({
          compilerOptions: {
            plugins: [
              {
                name: "@effect/language-service",
                diagnosticSeverity: {
                  floatingEffect: "warning"
                }
              }
            ]
          }
        }, null, 2),
        diagnosticSeverities: null
      })

      const tsconfigAction = result.codeActions.find((action) =>
        action.changes.some((change) => change.fileName.includes("tsconfig.json"))
      )
      expect(tsconfigAction).toBeDefined()
      const rendered = tsconfigAction!.changes.flatMap((change) => change.textChanges.map((textChange) => textChange.newText)).join("\n")
      expect(rendered).not.toContain("diagnosticSeverity")
    })
  })
})
