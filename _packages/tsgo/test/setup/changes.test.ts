import { describe, it, expect } from "vitest"
import * as Option from "effect/Option"
import * as ts from "typescript"
import { computeChanges, type ComputeChangesResult } from "../../src/cli/setup/changes.js"
import { assess } from "../../src/cli/setup/assessment.js"
import type { Assessment, Editor } from "../../src/cli/setup/types.js"

const TEST_TYPESCRIPT_VERSION = "7.1.0-dev.test"
const TEST_SCHEMA_PATH = "./node_modules/@effect/tsgo/schema.json"
const TEST_OXLINT_SCHEMA_PATH = "./node_modules/@effect/tsgo/oxlint-schema.json"

const ZED_BINARY_PATH =
  `./node_modules/@typescript/typescript-${process.platform}-${process.arch}/lib/${process.platform === "win32" ? "tsc.exe" : "tsc"}`

const ZED_TARGET_SETTINGS: Record<string, unknown> = {
  lsp: {
    "typescript-ls": {
      binary: {
        path: ZED_BINARY_PATH,
        arguments: ["--lsp", "--stdio"]
      }
    }
  },
  languages: {
    TypeScript: {
      language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "..."]
    },
    TSX: {
      language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "..."]
    }
  }
}

const applyTextChanges = (
  text: string,
  changes: ReadonlyArray<{ span: { start: number; length: number }; newText: string }>
) => [...changes]
  .sort((left, right) => right.span.start - left.span.start)
  .reduce(
    (result, change) =>
      result.slice(0, change.span.start) + change.newText + result.slice(change.span.start + change.span.length),
    text
  )

const parseJsonc = (text: string): Record<string, unknown> => {
  const sourceFile = ts.parseJsonText("/test/.zed/settings.json", text)
  expect((sourceFile as ts.JsonSourceFile & { readonly parseDiagnostics: ReadonlyArray<ts.Diagnostic> }).parseDiagnostics)
    .toEqual([])
  const errors: Array<ts.Diagnostic> = []
  const parsed = ts.convertToObject(sourceFile, errors) as Record<string, unknown>
  expect(errors).toEqual([])
  return parsed
}

const getZedSettingsChange = (result: ComputeChangesResult) =>
  result.codeActions
    .flatMap((action) => action.changes)
    .find((change) => change.fileName === "/test/.zed/settings.json")

/**
 * Helper to create an Assessment.Input and run assess() + computeChanges()
 */
function runComputeChanges(opts: {
  packageJsonText?: string
  tsconfigText?: string
  oxlintConfigText?: string | null
  vscodeSettingsText?: string | null
  zedSettingsText?: string | null
  editors?: ReadonlyArray<Editor>
  lspVersion?: { dependencyType: "dependencies" | "devDependencies"; version: string } | null
  typescriptVersion?: { dependencyType: "dependencies" | "devDependencies"; version: string; packageName?: string } | null
  oxlintVersion?: { dependencyType: "dependencies" | "devDependencies"; version: string } | null
  oxlintTsgolintVersion?: { dependencyType: "dependencies" | "devDependencies"; version: string } | null
  integrations?: ReadonlyArray<"typescript" | "oxlint">
  prepareScript?: boolean
  vscodeTargetSettings?: Record<string, unknown> | null
  zedTargetSettings?: Record<string, unknown> | null
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
    oxlintConfig: opts.oxlintConfigText != null
      ? Option.some({ fileName: "/test/.oxlintrc.json", text: opts.oxlintConfigText })
      : Option.none(),
    vscodeSettings: opts.vscodeSettingsText != null
      ? Option.some({ fileName: "/test/.vscode/settings.json", text: opts.vscodeSettingsText })
      : Option.none(),
    zedSettings: opts.zedSettingsText != null
      ? Option.some({ fileName: "/test/.zed/settings.json", text: opts.zedSettingsText })
      : Option.none()
  }

  const assessment = assess(input)

  const lspVersion = opts.lspVersion !== undefined
    ? (opts.lspVersion === null ? Option.none() : Option.some(opts.lspVersion))
    : Option.some({ dependencyType: "devDependencies" as const, version: "0.0.4" })

  const typescriptVersion = opts.typescriptVersion !== undefined
    ? (opts.typescriptVersion === null ? Option.none() : Option.some(opts.typescriptVersion))
    : Option.match(lspVersion, {
      onNone: () => Option.none(),
      onSome: (lsp) => Option.some({
        dependencyType: lsp.dependencyType,
        version: TEST_TYPESCRIPT_VERSION,
        packageName: "typescript"
      })
    })

  const vscodeTargetSettings = opts.vscodeTargetSettings !== undefined
    ? (opts.vscodeTargetSettings === null ? Option.none() : Option.some({ settings: opts.vscodeTargetSettings }))
    : Option.some({ settings: { "typescript.tsserver.experimental.enableProjectDiagnostics": true } })

  const zedTargetSettings = opts.zedTargetSettings !== undefined
    ? (opts.zedTargetSettings === null ? Option.none() : Option.some({ settings: opts.zedTargetSettings }))
    : Option.some({ settings: ZED_TARGET_SETTINGS })

  const target = {
    packageJson: {
      lspVersion,
      typescriptVersion,
      oxlintVersion: opts.oxlintVersion === undefined
        ? assessment.packageJson.oxlintVersion
        : opts.oxlintVersion === null ? Option.none() : Option.some(opts.oxlintVersion),
      oxlintTsgolintVersion: opts.oxlintTsgolintVersion === undefined
        ? assessment.packageJson.oxlintTsgolintVersion
        : opts.oxlintTsgolintVersion === null ? Option.none() : Option.some(opts.oxlintTsgolintVersion),
      prepareScript: opts.prepareScript ?? true,
      managePrepareScript: true,
      integrations: opts.integrations ?? (Option.isSome(lspVersion) ? ["typescript" as const] : [])
    },
    tsconfig: {
      schemaPath: Option.match(lspVersion, {
        onNone: () => Option.none(),
        onSome: () => Option.some(TEST_SCHEMA_PATH)
      }),
      diagnosticSeverities: opts.diagnosticSeverities === undefined
        ? Option.none()
        : opts.diagnosticSeverities === null
        ? Option.none()
        : Option.some(opts.diagnosticSeverities),
      manageIntegration: true
    },
    oxlintrcSchemaPath: (opts.integrations ?? []).includes("oxlint") && Option.isSome(assessment.oxlintConfig)
      ? Option.some(TEST_OXLINT_SCHEMA_PATH)
      : Option.none(),
    vscodeSettings: vscodeTargetSettings,
    zedSettings: zedTargetSettings,
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

  it("should assess typescript >= 7 from dependencies as the native backend", () => {
    const packageJsonText = JSON.stringify({
      name: "test-project",
      version: "1.0.0",
      dependencies: {
        "typescript": "^7.0.1-rc"
      }
    }, null, 2)

    const input: Assessment.Input = {
      packageJson: { fileName: "/test/package.json", text: packageJsonText },
      tsconfig: { fileName: "/test/tsconfig.json", text: "{}" },
      oxlintConfig: Option.none(),
      vscodeSettings: Option.none(),
      zedSettings: Option.none()
    }

    const assessment = assess(input)

    expect(assessment.packageJson.typescriptVersion).toEqual(Option.some({
      dependencyType: "dependencies",
      version: "^7.0.1-rc",
      packageName: "typescript"
    }))
  })

  it("should assess @typescript/native after typescript as the native backend", () => {
    const packageJsonText = JSON.stringify({
      name: "test-project",
      version: "1.0.0",
      devDependencies: {
        "@typescript/native": "npm:typescript@^7.0.2",
        "typescript": "npm:@typescript/typescript6@^6.0.2"
      }
    }, null, 2)

    const input: Assessment.Input = {
      packageJson: { fileName: "/test/package.json", text: packageJsonText },
      tsconfig: { fileName: "/test/tsconfig.json", text: "{}" },
      oxlintConfig: Option.none(),
      vscodeSettings: Option.none(),
      zedSettings: Option.none()
    }

    const assessment = assess(input)

    expect(assessment.packageJson.typescriptVersion).toEqual(Option.some({
      dependencyType: "devDependencies",
      version: "npm:typescript@^7.0.2",
      packageName: "@typescript/native"
    }))
  })

  it("should assess Oxlint dependencies independently", () => {
    const assessment = assess({
      packageJson: {
        fileName: "/test/package.json",
        text: JSON.stringify({
          dependencies: { oxlint: "^1.0.0" },
          devDependencies: { "oxlint-tsgolint": "^7.0.0" }
        })
      },
      tsconfig: { fileName: "/test/tsconfig.json", text: "{}" },
      oxlintConfig: Option.none(),
      vscodeSettings: Option.none(),
      zedSettings: Option.none()
    })

    expect(assessment.packageJson.oxlintVersion).toEqual(Option.some({
      dependencyType: "dependencies",
      version: "^1.0.0"
    }))
    expect(assessment.packageJson.oxlintTsgolintVersion).toEqual(Option.some({
      dependencyType: "devDependencies",
      version: "^7.0.0"
    }))
  })

  it.each(["dependencies", "devDependencies"] as const)(
    "should assess vite-plus from %s",
    (dependencyType) => {
      const assessment = assess({
        packageJson: {
          fileName: "/test/package.json",
          text: JSON.stringify({ [dependencyType]: { "vite-plus": "^0.2.8" } })
        },
        tsconfig: { fileName: "/test/tsconfig.json", text: "{}" },
        oxlintConfig: Option.none(),
        vscodeSettings: Option.none(),
        zedSettings: Option.none()
      })

      expect(assessment.packageJson.vitePlusVersion).toEqual(Option.some({
        dependencyType,
        version: "^0.2.8"
      }))
    }
  )

  it("should pin both Oxlint dependencies when enabling the integration", () => {
    const result = runComputeChanges({
      integrations: ["oxlint"],
      typescriptVersion: null,
      oxlintVersion: { dependencyType: "devDependencies", version: "1.77.0" },
      oxlintTsgolintVersion: { dependencyType: "devDependencies", version: "7.0.2001" }
    })
    const packageJsonChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/package.json")
    const insertedText = packageJsonChange?.textChanges.map((change) => change.newText).join("\n") ?? ""

    expect(insertedText).toContain('"oxlint": "1.77.0"')
    expect(insertedText).toContain('"oxlint-tsgolint": "7.0.2001"')
    expect(insertedText).toContain("effect-tsgo patch --no-typescript --oxlint")
  })

  it.each(["dependencies", "devDependencies"] as const)(
    "should not add Oxlint dependencies when vite-plus is in %s",
    (dependencyType) => {
      const packageJsonText = JSON.stringify({
        name: "test-project",
        [dependencyType]: {
          "vite-plus": "^0.2.8"
        }
      }, null, 2)
      const result = runComputeChanges({
        packageJsonText,
        integrations: ["oxlint"],
        typescriptVersion: null,
        oxlintVersion: { dependencyType, version: "1.77.0" },
        oxlintTsgolintVersion: { dependencyType, version: "7.0.2001" }
      })
      const packageJsonChange = result.codeActions
        .flatMap((action) => action.changes)
        .find((change) => change.fileName === "/test/package.json")
      const updated = applyTextChanges(packageJsonText, packageJsonChange?.textChanges ?? [])

      const parsed = JSON.parse(updated)
      expect(parsed[dependencyType]["vite-plus"]).toBe("^0.2.8")
      expect(parsed.dependencies?.oxlint).toBeUndefined()
      expect(parsed.dependencies?.["oxlint-tsgolint"]).toBeUndefined()
      expect(parsed.devDependencies?.oxlint).toBeUndefined()
      expect(parsed.devDependencies?.["oxlint-tsgolint"]).toBeUndefined()
      expect(updated).toContain("effect-tsgo patch --no-typescript --oxlint")
    }
  )

  it("should continue to pin explicit Oxlint dependencies alongside vite-plus", () => {
    const packageJsonText = JSON.stringify({
      name: "test-project",
      devDependencies: {
        "vite-plus": "^0.2.8",
        "oxlint": "1.76.0",
        "oxlint-tsgolint": "7.0.2000"
      }
    }, null, 2)
    const result = runComputeChanges({
      packageJsonText,
      integrations: ["oxlint"],
      typescriptVersion: null,
      oxlintVersion: { dependencyType: "devDependencies", version: "1.77.0" },
      oxlintTsgolintVersion: { dependencyType: "devDependencies", version: "7.0.2001" }
    })
    const packageJsonChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/package.json")
    const updated = JSON.parse(applyTextChanges(packageJsonText, packageJsonChange?.textChanges ?? []))

    expect(updated.devDependencies.oxlint).toBe("1.77.0")
    expect(updated.devDependencies["oxlint-tsgolint"]).toBe("7.0.2001")
  })

  it("should prepend the Effect Oxlint schema to an existing .oxlintrc.json", () => {
    const oxlintConfigText = JSON.stringify({
      plugins: ["typescript"],
      rules: { "no-debugger": "error" }
    }, null, 2)
    const result = runComputeChanges({
      oxlintConfigText,
      integrations: ["oxlint"],
      typescriptVersion: null,
      oxlintVersion: { dependencyType: "devDependencies", version: "1.77.0" },
      oxlintTsgolintVersion: { dependencyType: "devDependencies", version: "7.0.2001" }
    })
    const oxlintConfigChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/.oxlintrc.json")

    expect(oxlintConfigChange).toBeDefined()
    const updated = JSON.parse(applyTextChanges(oxlintConfigText, oxlintConfigChange!.textChanges))
    expect(Object.keys(updated)[0]).toBe("$schema")
    expect(updated.$schema).toBe(TEST_OXLINT_SCHEMA_PATH)
    expect(updated.rules).toEqual({ "no-debugger": "error" })
  })

  it("should replace an existing .oxlintrc.json schema when enabling Oxlint", () => {
    const oxlintConfigText = JSON.stringify({
      $schema: "./node_modules/oxlint/configuration_schema.json",
      rules: {}
    }, null, 2)
    const result = runComputeChanges({
      oxlintConfigText,
      integrations: ["oxlint"],
      typescriptVersion: null,
      oxlintVersion: { dependencyType: "devDependencies", version: "1.77.0" },
      oxlintTsgolintVersion: { dependencyType: "devDependencies", version: "7.0.2001" }
    })
    const oxlintConfigChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/.oxlintrc.json")

    expect(oxlintConfigChange).toBeDefined()
    const updated = JSON.parse(applyTextChanges(oxlintConfigText, oxlintConfigChange!.textChanges))
    expect(updated.$schema).toBe(TEST_OXLINT_SCHEMA_PATH)
  })

  it("should leave .oxlintrc.json unchanged when Oxlint is not selected", () => {
    const result = runComputeChanges({
      oxlintConfigText: JSON.stringify({ rules: {} }),
      integrations: ["typescript"]
    })

    expect(result.codeActions.flatMap((action) => action.changes)
      .some((change) => change.fileName === "/test/.oxlintrc.json")).toBe(false)
  })

  it("should remove TypeScript configuration for an Oxlint-only setup", () => {
    const result = runComputeChanges({
      tsconfigText: JSON.stringify({
        $schema: "./node_modules/@effect/tsgo/schema.json",
        compilerOptions: {
          plugins: [{ name: "@effect/language-service" }]
        }
      }, null, 2),
      integrations: ["oxlint"],
      typescriptVersion: null,
      oxlintVersion: { dependencyType: "devDependencies", version: "1.77.0" },
      oxlintTsgolintVersion: { dependencyType: "devDependencies", version: "7.0.2001" }
    })
    const tsconfigChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/tsconfig.json")
    const changedText = tsconfigChange?.textChanges.map((change) => change.newText).join("\n") ?? ""

    expect(tsconfigChange).toBeDefined()
    expect(changedText).not.toContain("@effect/language-service")
  })

  it("should insert a new $schema as the first tsconfig property", () => {
    const tsconfigText = JSON.stringify({
      extends: "./tsconfig.base.json",
      compilerOptions: { strict: true }
    }, null, 2)
    const result = runComputeChanges({ tsconfigText })
    const tsconfigChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/tsconfig.json")

    expect(tsconfigChange).toBeDefined()
    const updated = JSON.parse(applyTextChanges(tsconfigText, tsconfigChange!.textChanges))
    expect(Object.keys(updated)[0]).toBe("$schema")
  })

  it("should preserve an unrelated tsconfig $schema when disabling TypeScript", () => {
    const result = runComputeChanges({
      tsconfigText: JSON.stringify({
        $schema: "https://json.schemastore.org/tsconfig",
        compilerOptions: {
          plugins: [{ name: "@effect/language-service" }]
        }
      }, null, 2),
      lspVersion: null,
      integrations: []
    })
    const tsconfigChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/tsconfig.json")

    expect(tsconfigChange).toBeDefined()
    expect(applyTextChanges(JSON.stringify({
      $schema: "https://json.schemastore.org/tsconfig",
      compilerOptions: {
        plugins: [{ name: "@effect/language-service" }]
      }
    }, null, 2), tsconfigChange!.textChanges)).toContain("https://json.schemastore.org/tsconfig")
  })

  it("should remove the Effect schema when compilerOptions is missing", () => {
    const tsconfigText = JSON.stringify({
      $schema: TEST_SCHEMA_PATH,
      extends: "./tsconfig.base.json"
    }, null, 2)
    const result = runComputeChanges({
      tsconfigText,
      lspVersion: null,
      integrations: []
    })
    const tsconfigChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/tsconfig.json")

    expect(tsconfigChange).toBeDefined()
    expect(applyTextChanges(tsconfigText, tsconfigChange!.textChanges)).not.toContain("$schema")
  })

  it("should not assess typescript < 7 as the native backend", () => {
    const packageJsonText = JSON.stringify({
      name: "test-project",
      version: "1.0.0",
      dependencies: {
        "typescript": "^5.9.2"
      }
    }, null, 2)

    const input: Assessment.Input = {
      packageJson: { fileName: "/test/package.json", text: packageJsonText },
      tsconfig: { fileName: "/test/tsconfig.json", text: "{}" },
      oxlintConfig: Option.none(),
      vscodeSettings: Option.none(),
      zedSettings: Option.none()
    }

    const assessment = assess(input)

    expect(assessment.packageJson.typescriptVersion).toEqual(Option.none())
  })

  it("should add typescript when installing the LSP if a native backend is missing", () => {
    const result = runComputeChanges({
      packageJsonText: JSON.stringify({
        name: "test-project",
        version: "1.0.0",
        devDependencies: {}
      }, null, 2),
      lspVersion: { dependencyType: "devDependencies", version: "0.0.4" },
      typescriptVersion: {
        dependencyType: "devDependencies",
        version: TEST_TYPESCRIPT_VERSION,
        packageName: "typescript"
      },
      prepareScript: false,
      editors: []
    })

    const packageJsonChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/package.json")

    expect(packageJsonChange).toBeDefined()
    expect(packageJsonChange?.textChanges.some((change) => change.newText.includes('"typescript"'))).toBe(true)
    expect(result.codeActions[0]?.description).toContain(`Add typescript@${TEST_TYPESCRIPT_VERSION} to devDependencies`)
  })

  it("should add the selected typescript backend", () => {
    const result = runComputeChanges({
      packageJsonText: JSON.stringify({
        name: "test-project",
        version: "1.0.0",
        devDependencies: {}
      }, null, 2),
      lspVersion: { dependencyType: "devDependencies", version: "0.0.4" },
      typescriptVersion: { dependencyType: "devDependencies", version: "npm:typescript@^7.0.2", packageName: "@typescript/native" },
      prepareScript: false,
      editors: []
    })

    const packageJsonChange = result.codeActions
      .flatMap((action) => action.changes)
      .find((change) => change.fileName === "/test/package.json")

    expect(packageJsonChange).toBeDefined()
    expect(packageJsonChange?.textChanges.some((change) => change.newText.includes('"@typescript/native"'))).toBe(true)
    expect(result.codeActions[0]?.description).toContain("Add @typescript/native@npm:typescript@^7.0.2 to devDependencies")
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

    it("should serialize array settings when modifying existing vscode settings", () => {
      const result = runComputeChanges({
        vscodeSettingsText: JSON.stringify({ "editor.formatOnSave": true }, null, 2),
        vscodeTargetSettings: {
          "js/ts.tsdk.additionalLocations": ["./node_modules/typescript/bin"]
        }
      })

      const vscodeChange = result.codeActions
        .flatMap((action) => action.changes)
        .find((change) => change.fileName.includes("settings.json"))

      expect(vscodeChange?.textChanges[0]?.newText).toContain('["./node_modules/typescript/bin"]')
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

  describe("Zed settings", () => {
    it("merges JSONC without clobbering siblings or comments, normalizes servers, and is idempotent", () => {
      const zedSettingsText = `{
  // Keep project-wide settings.
  "theme": "Ayu",
  "lsp": {
    "eslint": {
      "settings": {
        "workingDirectories": [{ "mode": "auto" }]
      }
    },
    // Keep server-specific initialization.
    "typescript-language-server": {
      "initialization_options": {
        "preferences": {
          "includeInlayParameterNameHints": "all"
        }
      },
      "binary": {
        "path": "/opt/typescript-language-server",
        "arguments": ["--stdio"]
      }
    }
  },
  "languages": {
    "TypeScript": {
      // Keep formatter and code actions.
      "formatter": {
        "external": {
          "command": "prettier",
          "arguments": ["--stdin-filepath", "{buffer_path}"]
        }
      },
      "code_actions_on_format": {
        "source.fixAll.eslint": true
      },
      "language_servers": [
        "!typescript-language-server",
        "eslint",
        "!eslint",
        "eslint",
        "...",
        "!vtsls"
      ]
    },
    "TSX": {
      "formatter": "language_server",
      "language_servers": ["biome", "...", "biome"]
    },
    "JavaScript": {
      "language_servers": ["eslint", "..."]
    }
  }
}
`
      const firstResult = runComputeChanges({
        zedSettingsText,
        editors: ["zed"],
        vscodeTargetSettings: null
      })
      const firstChange = getZedSettingsChange(firstResult)
      if (firstChange === undefined) {
        throw new Error("Expected an existing .zed/settings.json change")
      }
      const mergedText = applyTextChanges(zedSettingsText, firstChange.textChanges)

      expect(firstResult.messages.slice(-3)).toEqual([
        "Zed:",
        "  Restart Zed to activate the TypeScript language server.",
        ""
      ])

      expect(parseJsonc(mergedText)).toEqual({
        theme: "Ayu",
        lsp: {
          eslint: {
            settings: {
              workingDirectories: [{ mode: "auto" }]
            }
          },
          "typescript-language-server": {
            initialization_options: {
              preferences: {
                includeInlayParameterNameHints: "all"
              }
            },
            binary: {
              path: "/opt/typescript-language-server",
              arguments: ["--stdio"]
            }
          },
          "typescript-ls": {
            binary: {
              path: ZED_BINARY_PATH,
              arguments: ["--lsp", "--stdio"]
            }
          }
        },
        languages: {
          TypeScript: {
            formatter: {
              external: {
                command: "prettier",
                arguments: ["--stdin-filepath", "{buffer_path}"]
              }
            },
            code_actions_on_format: {
              "source.fixAll.eslint": true
            },
            language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "eslint", "!eslint", "..."]
          },
          TSX: {
            formatter: "language_server",
            language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "biome", "..."]
          },
          JavaScript: {
            language_servers: ["eslint", "..."]
          }
        }
      })
      expect(mergedText).toContain("// Keep project-wide settings.")
      expect(mergedText).toContain("// Keep server-specific initialization.")
      expect(mergedText).toContain("// Keep formatter and code actions.")

      const secondResult = runComputeChanges({
        zedSettingsText: mergedText,
        editors: ["zed"],
        vscodeTargetSettings: null
      })
      expect(getZedSettingsChange(secondResult)).toBeUndefined()
    })

    it("does not opt explicit language server lists back into Zed defaults", () => {
      const zedSettingsText = JSON.stringify({
        lsp: {
          "typescript-language-server": {
            binary: {
              path: "./node_modules/.bin/tsc",
              arguments: ["--lsp", "--stdio"]
            }
          }
        },
        languages: {
          TypeScript: {
            language_servers: ["eslint", "!typescript-language-server"]
          },
          TSX: {
            language_servers: ["biome", "typescript-language-server", "typescript-language-server", "!vtsls"]
          }
        }
      }, null, 2)
      const result = runComputeChanges({
        zedSettingsText,
        editors: ["zed"],
        vscodeTargetSettings: null
      })
      const change = getZedSettingsChange(result)
      if (change === undefined) {
        throw new Error("Expected explicit Zed language server lists to be normalized")
      }
      const mergedText = applyTextChanges(zedSettingsText, change.textChanges)

      expect(parseJsonc(mergedText)).toEqual({
        lsp: {
          "typescript-language-server": {
            binary: {
              path: "./node_modules/.bin/tsc",
              arguments: ["--lsp", "--stdio"]
            }
          },
          "typescript-ls": {
            binary: {
              path: ZED_BINARY_PATH,
              arguments: ["--lsp", "--stdio"]
            }
          }
        },
        languages: {
          TypeScript: {
            language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "eslint"]
          },
          TSX: {
            language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "biome"]
          }
        }
      })
    })

    it("aborts the entire Zed edit without restart guidance when a required container has an incompatible shape", () => {
      const zedSettingsText = `{
  // User-owned values with incompatible shapes must not be replaced.
  "lsp": [],
  "languages": {
    "TypeScript": "custom-language-config",
    "JavaScript": {
      "language_servers": ["eslint", "..."]
    }
  }
}
`
      const result = runComputeChanges({
        zedSettingsText,
        editors: ["zed"],
        vscodeTargetSettings: null
      })

      expect(getZedSettingsChange(result)).toBeUndefined()
      expect(result.messages).toEqual([
        "`package.json` changed. Run your package manager's install command (for example, `pnpm install`, `npm install`, `yarn install`, or `bun install`).",
        "Unable to update .zed/settings.json: lsp must be an object.",
        "Run `effect-tsgo patch --typescript --no-oxlint` to complete the installation.",
        ""
      ])
      expect(result.messages).not.toContain("  Restart Zed to activate the TypeScript language server.")
    })

    it.each([
      ["Zed is not selected", [] as ReadonlyArray<Editor>, undefined, null],
      ["the TypeScript LSP is disabled", ["zed"] as ReadonlyArray<Editor>, null, "{}"]
    ] as const)("does not touch .zed/settings.json when %s", (_, editors, lspVersion, zedSettingsText) => {
      const result = runComputeChanges({
        zedSettingsText,
        editors,
        lspVersion,
        vscodeTargetSettings: null
      })

      expect(getZedSettingsChange(result)).toBeUndefined()
    })
  })

  describe("post-apply messages", () => {
    const installMessage = "`package.json` changed. Run your package manager's install command " +
      "(for example, `pnpm install`, `npm install`, `yarn install`, or `bun install`)."

    it("should recommend installing when package.json changes", () => {
      const result = runComputeChanges({})

      expect(result.messages).toContain(installMessage)
    })

    it("should not recommend installing when package.json is unchanged", () => {
      const packageJsonText = JSON.stringify({
        name: "test-project",
        version: "1.0.0",
        devDependencies: {
          "@effect/tsgo": "0.0.4",
          "typescript": TEST_TYPESCRIPT_VERSION
        },
        scripts: {
          prepare: "effect-tsgo patch --typescript --no-oxlint"
        }
      }, null, 2)
      const result = runComputeChanges({ packageJsonText })

      expect(result.codeActions.some((action) =>
        action.changes.some((change) => change.fileName === "/test/package.json")
      )).toBe(false)
      expect(result.messages).not.toContain(installMessage)
    })

    it("should include patch message when installing", () => {
      const result = runComputeChanges({})

      expect(result.messages).toContain(
        "Run `effect-tsgo patch --typescript --no-oxlint` to complete the installation."
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
        "Run `effect-tsgo unpatch --typescript --no-oxlint` to restore the original integrations."
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
      expect(rendered).toContain(TEST_SCHEMA_PATH)
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
      expect(rendered).toContain(TEST_SCHEMA_PATH)
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

  describe("JSON indentation", () => {
    it.each([
      ["two spaces", "  ", "\n"],
      ["four spaces", "    ", "\n"],
      ["tabs", "\t", "\n"],
      ["CRLF newlines", "  ", "\r\n"]
    ])("should preserve %s in package.json", (_, indentation, newLine) => {
      const packageJsonText = [
        "{",
        `${indentation}"name": "test-project",`,
        `${indentation}"devDependencies": {`,
        `${indentation.repeat(2)}"typescript": "${TEST_TYPESCRIPT_VERSION}"`,
        `${indentation}}`,
        "}"
      ].join(newLine)
      const result = runComputeChanges({
        packageJsonText,
        typescriptVersion: null,
        prepareScript: false,
        editors: [],
        vscodeTargetSettings: null
      })
      const change = result.codeActions
        .flatMap((action) => action.changes)
        .find((change) => change.fileName === "/test/package.json")
      const updated = applyTextChanges(packageJsonText, change?.textChanges ?? [])

      expect(updated).toContain(`${newLine}${indentation.repeat(2)}"@effect/tsgo": "0.0.4"`)
      expect(() => JSON.parse(updated)).not.toThrow()
    })

    it.each([
      ["two spaces", "  "],
      ["four spaces", "    "],
      ["tabs", "\t"]
    ])("should preserve %s in tsconfig.json", (_, indentation) => {
      const tsconfigText = [
        "{",
        `${indentation}"compilerOptions": {`,
        `${indentation.repeat(2)}"strict": true`,
        `${indentation}}`,
        "}"
      ].join("\n")
      const result = runComputeChanges({
        tsconfigText,
        prepareScript: false,
        editors: [],
        vscodeTargetSettings: null
      })
      const change = result.codeActions
        .flatMap((action) => action.changes)
        .find((change) => change.fileName === "/test/tsconfig.json")
      const updated = applyTextChanges(tsconfigText, change?.textChanges ?? [])

      expect(updated).toContain(`\n${indentation}"$schema"`)
      expect(updated).toContain(`\n${indentation.repeat(2)}"plugins"`)
      expect(updated).toContain(`\n${indentation.repeat(4)}"name": "@effect/language-service"`)
      expect(() => JSON.parse(updated)).not.toThrow()
    })

    it.each([
      { layout: "inline", lines: ["  \"devDependencies\": {}"] },
      { layout: "multiline", lines: ["  \"devDependencies\": {", "  }"] }
    ])("should indent properties added to an $layout empty object", ({ lines }) => {
      const packageJsonText = [
        "{",
        "  \"name\": \"test-project\",",
        ...lines,
        "}"
      ].join("\n")
      const result = runComputeChanges({
        packageJsonText,
        prepareScript: false,
        editors: [],
        vscodeTargetSettings: null
      })
      const change = result.codeActions
        .flatMap((action) => action.changes)
        .find((change) => change.fileName === "/test/package.json")
      const updated = applyTextChanges(packageJsonText, change?.textChanges ?? [])

      expect(updated).toBe([
        "{",
        "  \"name\": \"test-project\",",
        "  \"devDependencies\": {",
        `    "typescript": "${TEST_TYPESCRIPT_VERSION}",`,
        "    \"@effect/tsgo\": \"0.0.4\"",
        "  }",
        "}"
      ].join("\n"))
      expect(() => JSON.parse(updated)).not.toThrow()
    })
  })
})
