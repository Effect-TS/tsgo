import * as nodePath from "node:path"
import * as Option from "effect/Option"
import * as ts from "typescript"
import type { Assessment, SetupCodeAction, Target } from "./types.js"
import { LSP_PACKAGE_NAME, LSP_PLUGIN_NAME, PATCH_COMMAND, TSCONFIG_SCHEMA_URL } from "./consts.js"
import type { RuleSeverity } from "./rule-info.js"

interface ComputeFileChangesResult {
  readonly codeActions: ReadonlyArray<SetupCodeAction>
  readonly messages: ReadonlyArray<string>
}

function emptyFileChangesResult(): ComputeFileChangesResult {
  return { codeActions: [], messages: [] }
}

export interface ComputeChangesResult {
  readonly codeActions: ReadonlyArray<SetupCodeAction>
  readonly messages: ReadonlyArray<string>
}

/**
 * Find a property in an object literal expression by name
 */
function findPropertyInObject(
  obj: ts.ObjectLiteralExpression,
  propertyName: string
): ts.PropertyAssignment | undefined {
  for (const prop of obj.properties) {
    if (ts.isPropertyAssignment(prop)) {
      const name = prop.name
      if (ts.isIdentifier(name) && ts.idText(name) === propertyName) {
        return prop
      }
      if (ts.isStringLiteral(name) && name.text === propertyName) {
        return prop
      }
    }
  }
  return undefined
}

/**
 * Get the root object literal from a JSON source file
 */
function getRootObject(
  sourceFile: ts.JsonSourceFile
): ts.ObjectLiteralExpression | undefined {
  if (sourceFile.statements.length === 0) return undefined
  const statement = sourceFile.statements[0]
  if (!ts.isExpressionStatement(statement)) return undefined
  const expr = statement.expression
  if (!ts.isObjectLiteralExpression(expr)) return undefined
  return expr
}

/**
 * Delete a node from a list (array or object properties), handling commas properly
 */
function deleteNodeFromList<T extends ts.Node>(
  tracker: any,
  sourceFile: ts.SourceFile,
  nodeArray: ts.NodeArray<T>,
  nodeToDelete: T
) {
  const index = nodeArray.indexOf(nodeToDelete)
  if (index === -1) return

  if (index === 0 && nodeArray.length > 1) {
    const secondElement = nodeArray[1]
    tracker.deleteRange(sourceFile, { pos: nodeToDelete.pos, end: secondElement.pos })
  } else if (index > 0) {
    const previousElement = nodeArray[index - 1]
    tracker.deleteRange(sourceFile, { pos: previousElement.end, end: nodeToDelete.end })
  } else {
    tracker.delete(sourceFile, nodeToDelete)
  }
}

/**
 * Insert a node at the end of a list (array or object properties), handling commas properly
 */
function insertNodeAtEndOfList<T extends ts.Node>(
  tracker: any,
  sourceFile: ts.SourceFile,
  nodeArray: ts.NodeArray<T>,
  newNode: T
) {
  if (nodeArray.length === 0) {
    tracker.insertNodeAt(sourceFile, nodeArray.pos + 1, newNode, { suffix: "\n" })
  } else {
    const lastElement = nodeArray[nodeArray.length - 1]
    tracker.insertNodeAt(sourceFile, lastElement.end, newNode, { prefix: ",\n" })
  }
}

function createDiagnosticSeverityObject(
  severities: Record<string, RuleSeverity>
): ts.ObjectLiteralExpression {
  const entries = Object.entries(severities).sort(([a], [b]) => a.localeCompare(b))
  return ts.factory.createObjectLiteralExpression(
    entries.map(([name, severity]) =>
      ts.factory.createPropertyAssignment(
        ts.factory.createStringLiteral(name),
        ts.factory.createStringLiteral(severity)
      )
    ),
    true
  )
}

function createLspPluginObject(target: Target.TsConfig): ts.ObjectLiteralExpression {
  const properties: Array<ts.PropertyAssignment> = [
    ts.factory.createPropertyAssignment(
      ts.factory.createStringLiteral("name"),
      ts.factory.createStringLiteral(LSP_PLUGIN_NAME)
    )
  ]
  if (Option.isSome(target.diagnosticSeverities)) {
    properties.push(
      ts.factory.createPropertyAssignment(
        ts.factory.createStringLiteral("diagnosticSeverity"),
        createDiagnosticSeverityObject(target.diagnosticSeverities.value)
      )
    )
  }
  return ts.factory.createObjectLiteralExpression(properties, true)
}

/**
 * Create a minimal LanguageServiceHost for use with ChangeTracker
 */
function createMinimalHost(): ts.LanguageServiceHost {
  return {
    getCompilationSettings: () => ({}),
    getScriptFileNames: () => [],
    getScriptVersion: () => "1",
    getScriptSnapshot: () => undefined,
    getCurrentDirectory: () => "",
    getDefaultLibFileName: () => "lib.d.ts",
    fileExists: () => false,
    readFile: () => undefined
  }
}

// Access internal TypeScript APIs not exposed in public type definitions
const tsInternal = ts as any

/**
 * Create a ChangeTracker context
 */
function createTrackerContext() {
  const host = createMinimalHost()
  const formatOptions = { indentSize: 2, tabSize: 2 } as ts.EditorSettings
  const formatContext = tsInternal.formatting.getFormatContext(formatOptions, host)
  const preferences = {} as ts.UserPreferences
  return { host, formatContext, preferences }
}

/**
 * Compute package.json changes using ChangeTracker
 */
const computePackageJsonChanges = (
  current: Assessment.PackageJson,
  target: Target.PackageJson
): ComputeFileChangesResult => {
  const descriptions: Array<string> = []
  const messages: Array<string> = []

  const rootObj = getRootObject(current.sourceFile)
  if (!rootObj) {
    return emptyFileChangesResult()
  }

  const ctx = createTrackerContext()

  const fileChanges = tsInternal.textChanges.ChangeTracker.with(
    ctx,
    (tracker: any) => {
      // Handle @effect/tsgo dependency
      if (Option.isSome(target.lspVersion)) {
        const targetDepType = target.lspVersion.value.dependencyType
        const targetVersion = target.lspVersion.value.version

        if (Option.isSome(current.lspVersion)) {
          const currentDepType = current.lspVersion.value.dependencyType
          const currentVersion = current.lspVersion.value.version

          if (currentDepType !== targetDepType) {
            // Move from one dependency type to another
            descriptions.push(`Move ${LSP_PACKAGE_NAME} from ${currentDepType} to ${targetDepType}`)

            // Remove from old location
            const oldDepsProperty = findPropertyInObject(rootObj, currentDepType)
            if (oldDepsProperty && ts.isObjectLiteralExpression(oldDepsProperty.initializer)) {
              const lspProperty = findPropertyInObject(oldDepsProperty.initializer, LSP_PACKAGE_NAME)
              if (lspProperty) {
                deleteNodeFromList(tracker, current.sourceFile, oldDepsProperty.initializer.properties, lspProperty)
              }
            }

            // Add to new location
            const newDepsProperty = findPropertyInObject(rootObj, targetDepType)
            const newLspProp = ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral(LSP_PACKAGE_NAME),
              ts.factory.createStringLiteral(targetVersion)
            )

            if (!newDepsProperty) {
              const newDepsProp = ts.factory.createPropertyAssignment(
                ts.factory.createStringLiteral(targetDepType),
                ts.factory.createObjectLiteralExpression([newLspProp], false)
              )
              insertNodeAtEndOfList(tracker, current.sourceFile, rootObj.properties, newDepsProp)
            } else if (ts.isObjectLiteralExpression(newDepsProperty.initializer)) {
              insertNodeAtEndOfList(tracker, current.sourceFile, newDepsProperty.initializer.properties, newLspProp)
            }
          } else if (currentVersion !== targetVersion) {
            // Same dependency type, just update version
            descriptions.push(`Update ${LSP_PACKAGE_NAME} from ${currentVersion} to ${targetVersion}`)

            const depsProperty = findPropertyInObject(rootObj, targetDepType)
            if (depsProperty && ts.isObjectLiteralExpression(depsProperty.initializer)) {
              const lspProperty = findPropertyInObject(depsProperty.initializer, LSP_PACKAGE_NAME)
              if (lspProperty && ts.isStringLiteral(lspProperty.initializer)) {
                tracker.replaceNode(
                  current.sourceFile,
                  lspProperty.initializer,
                  ts.factory.createStringLiteral(targetVersion)
                )
              }
            }
          }
        } else {
          // LSP not currently installed, add it
          descriptions.push(`Add ${LSP_PACKAGE_NAME}@${targetVersion} to ${targetDepType}`)

          const depsProperty = findPropertyInObject(rootObj, targetDepType)

          if (!depsProperty) {
            const newDepsProp = ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral(targetDepType),
              ts.factory.createObjectLiteralExpression([
                ts.factory.createPropertyAssignment(
                  ts.factory.createStringLiteral(LSP_PACKAGE_NAME),
                  ts.factory.createStringLiteral(targetVersion)
                )
              ], false)
            )
            insertNodeAtEndOfList(tracker, current.sourceFile, rootObj.properties, newDepsProp)
          } else if (ts.isObjectLiteralExpression(depsProperty.initializer)) {
            const newLspProp = ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral(LSP_PACKAGE_NAME),
              ts.factory.createStringLiteral(targetVersion)
            )
            insertNodeAtEndOfList(tracker, current.sourceFile, depsProperty.initializer.properties, newLspProp)
          }
        }
      } else if (Option.isSome(current.lspVersion)) {
        // User wants to remove LSP
        descriptions.push(`Remove ${LSP_PACKAGE_NAME} from dependencies`)

        const currentDepType = current.lspVersion.value.dependencyType
        const depsProperty = findPropertyInObject(rootObj, currentDepType)

        if (depsProperty && ts.isObjectLiteralExpression(depsProperty.initializer)) {
          const lspProperty = findPropertyInObject(depsProperty.initializer, LSP_PACKAGE_NAME)
          if (lspProperty) {
            deleteNodeFromList(tracker, current.sourceFile, depsProperty.initializer.properties, lspProperty)
          }
        }
      }

      // Handle prepare script
      if (target.prepareScript && Option.isSome(target.lspVersion)) {
        const scriptsProperty = findPropertyInObject(rootObj, "scripts")

        if (!scriptsProperty) {
          descriptions.push("Add scripts section with prepare script")

          const newScriptsProp = ts.factory.createPropertyAssignment(
            ts.factory.createStringLiteral("scripts"),
            ts.factory.createObjectLiteralExpression([
              ts.factory.createPropertyAssignment(
                ts.factory.createStringLiteral("prepare"),
                ts.factory.createStringLiteral(PATCH_COMMAND)
              )
            ], false)
          )
          insertNodeAtEndOfList(tracker, current.sourceFile, rootObj.properties, newScriptsProp)
        } else if (ts.isObjectLiteralExpression(scriptsProperty.initializer)) {
          const prepareProperty = findPropertyInObject(scriptsProperty.initializer, "prepare")

          if (!prepareProperty) {
            descriptions.push("Add prepare script")

            const newPrepareProp = ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral("prepare"),
              ts.factory.createStringLiteral(PATCH_COMMAND)
            )
            insertNodeAtEndOfList(tracker, current.sourceFile, scriptsProperty.initializer.properties, newPrepareProp)
          } else if (Option.isSome(current.prepareScript) && !current.prepareScript.value.hasPatch) {
            // Modify existing prepare script to add patch command
            descriptions.push("Update prepare script to include patch command")

            const currentScript = current.prepareScript.value.script
            const newScript = `${currentScript} && ${PATCH_COMMAND}`
            tracker.replaceNode(
              current.sourceFile,
              prepareProperty.initializer,
              ts.factory.createStringLiteral(newScript)
            )
          }
        }
      } else if (
        Option.isNone(target.lspVersion) && Option.isSome(current.prepareScript) &&
        current.prepareScript.value.hasPatch
      ) {
        // User wants to remove LSP and prepare script has patch command
        const scriptsProperty = findPropertyInObject(rootObj, "scripts")
        if (scriptsProperty && ts.isObjectLiteralExpression(scriptsProperty.initializer)) {
          const prepareProperty = findPropertyInObject(scriptsProperty.initializer, "prepare")
          if (prepareProperty && ts.isStringLiteral(prepareProperty.initializer)) {
            const currentScript = current.prepareScript.value.script
            const hasMultipleCommands = currentScript.includes("&&") || currentScript.includes(";")

            if (hasMultipleCommands) {
              descriptions.push("Remove effect-tsgo patch command from prepare script")
              messages.push(
                "WARNING: Your prepare script contained multiple commands. " +
                  "I attempted to automatically remove only the 'effect-tsgo patch' command. " +
                  "Please verify that the prepare script is correct after this change."
              )

              const newScript = currentScript
                .replace(/\s*&&\s*effect-tsgo\s+patch/g, "")
                .replace(/effect-tsgo\s+patch\s*&&\s*/g, "")
                .replace(/\s*;\s*effect-tsgo\s+patch/g, "")
                .replace(/effect-tsgo\s+patch\s*;\s*/g, "")
                .trim()

              tracker.replaceNode(
                current.sourceFile,
                prepareProperty.initializer,
                ts.factory.createStringLiteral(newScript)
              )
            } else {
              descriptions.push("Remove prepare script with patch command")
              deleteNodeFromList(tracker, current.sourceFile, scriptsProperty.initializer.properties, prepareProperty)
            }
          }
        }
      }
    }
  )

  const fileChange = fileChanges.find((fc: ts.FileTextChanges) => fc.fileName === current.path)
  const changes = fileChange ? fileChange.textChanges : []

  if (changes.length === 0) {
    return { codeActions: [], messages }
  }

  return {
    codeActions: [{
      description: descriptions.join("; "),
      changes: [{
        fileName: current.path,
        textChanges: changes,
        isNewFile: false
      }]
    }],
    messages
  }
}

/**
 * Compute tsconfig.json changes using ChangeTracker
 */
const computeTsConfigChanges = (
  current: Assessment.TsConfig,
  target: Target.TsConfig,
  lspVersion: Option.Option<{ readonly dependencyType: "dependencies" | "devDependencies"; readonly version: string }>
): ComputeFileChangesResult => {
  const descriptions: Array<string> = []
  const messages: Array<string> = []

  const rootObj = getRootObject(current.sourceFile)
  if (!rootObj) {
    return emptyFileChangesResult()
  }

  const compilerOptionsProperty = findPropertyInObject(rootObj, "compilerOptions")
  if (!compilerOptionsProperty || !ts.isObjectLiteralExpression(compilerOptionsProperty.initializer)) {
    // No compilerOptions — if we're removing LSP there's nothing to do
    if (Option.isNone(lspVersion)) {
      return emptyFileChangesResult()
    }

    // Create compilerOptions with the plugin entry
    const ctx = createTrackerContext()

    const fileChanges = tsInternal.textChanges.ChangeTracker.with(
      ctx,
      (tracker: any) => {
        const schemaProperty = findPropertyInObject(rootObj, "$schema")
        const shouldAddSchema = !schemaProperty
        const shouldUpdateSchema = !!schemaProperty && (
          !ts.isStringLiteral(schemaProperty.initializer) || schemaProperty.initializer.text !== TSCONFIG_SCHEMA_URL
        )

        if (shouldAddSchema) {
          descriptions.push("Add $schema to tsconfig")
        } else if (shouldUpdateSchema) {
          descriptions.push("Update $schema in tsconfig")
        }

        descriptions.push(`Add compilerOptions with ${LSP_PLUGIN_NAME} plugin`)

        const schemaPropertyAssignment = ts.factory.createPropertyAssignment(
          ts.factory.createStringLiteral("$schema"),
          ts.factory.createStringLiteral(TSCONFIG_SCHEMA_URL)
        )

        const compilerOptionsAssignment = ts.factory.createPropertyAssignment(
          ts.factory.createStringLiteral("compilerOptions"),
          ts.factory.createObjectLiteralExpression([
            ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral("plugins"),
              ts.factory.createArrayLiteralExpression([createLspPluginObject(target)], true)
            )
          ], true)
        )

        // Rebuild the root object preserving existing properties, updating/adding $schema, appending compilerOptions
        const nextProperties: Array<ts.ObjectLiteralElementLike> = rootObj.properties.map((property) => {
          if (schemaProperty && property === schemaProperty) {
            return schemaPropertyAssignment
          }
          return property
        })

        if (shouldAddSchema) {
          nextProperties.push(schemaPropertyAssignment)
        }
        nextProperties.push(compilerOptionsAssignment)

        tracker.replaceNode(
          current.sourceFile,
          rootObj,
          ts.factory.createObjectLiteralExpression(nextProperties, true)
        )
      }
    )

    const fileChange = fileChanges.find((fc: ts.FileTextChanges) => fc.fileName === current.sourceFile.fileName)
    const changes = fileChange ? fileChange.textChanges : []
    if (changes.length === 0) {
      return { codeActions: [], messages }
    }

    return {
      codeActions: [{
        description: descriptions.join("; "),
        changes: [{
          fileName: current.sourceFile.fileName,
          textChanges: changes,
          isNewFile: false
        }]
      }],
      messages
    }
  }

  const compilerOptions = compilerOptionsProperty.initializer

  const ctx = createTrackerContext()

  const fileChanges = tsInternal.textChanges.ChangeTracker.with(
    ctx,
    (tracker: any) => {
      const schemaProperty = findPropertyInObject(rootObj, "$schema")
      const pluginsProperty = findPropertyInObject(compilerOptions, "plugins")
      const schemaPropertyAssignment = ts.factory.createPropertyAssignment(
        ts.factory.createStringLiteral("$schema"),
        ts.factory.createStringLiteral(TSCONFIG_SCHEMA_URL)
      )

      if (Option.isNone(lspVersion)) {
        // User wants to remove LSP
        if (schemaProperty) {
          descriptions.push("Remove $schema from tsconfig")
          deleteNodeFromList(tracker, current.sourceFile, rootObj.properties, schemaProperty)
        }

        if (pluginsProperty && ts.isArrayLiteralExpression(pluginsProperty.initializer)) {
          const pluginsArray = pluginsProperty.initializer

          const lspPluginElement = pluginsArray.elements.find((element) => {
            if (ts.isObjectLiteralExpression(element)) {
              const nameProperty = findPropertyInObject(element, "name")
              if (nameProperty && ts.isStringLiteral(nameProperty.initializer)) {
                return nameProperty.initializer.text === LSP_PLUGIN_NAME
              }
            }
            return false
          })

          if (lspPluginElement) {
            descriptions.push(`Remove ${LSP_PLUGIN_NAME} plugin from tsconfig`)
            deleteNodeFromList(tracker, current.sourceFile, pluginsArray.elements, lspPluginElement)
          }
        }
      } else {
        // User wants to add/keep LSP
        if (!schemaProperty) {
          descriptions.push("Add $schema to tsconfig")
          insertNodeAtEndOfList(tracker, current.sourceFile, rootObj.properties, schemaPropertyAssignment)
        } else if (
          !ts.isStringLiteral(schemaProperty.initializer) ||
          schemaProperty.initializer.text !== TSCONFIG_SCHEMA_URL
        ) {
          descriptions.push("Update $schema in tsconfig")
          tracker.replaceNode(current.sourceFile, schemaProperty.initializer, schemaPropertyAssignment.initializer)
        }

        const pluginObject = createLspPluginObject(target)

        if (!pluginsProperty) {
          descriptions.push(`Add plugins array with ${LSP_PLUGIN_NAME} plugin`)

          const newPluginsProp = ts.factory.createPropertyAssignment(
            ts.factory.createStringLiteral("plugins"),
            ts.factory.createArrayLiteralExpression([pluginObject], true)
          )
          insertNodeAtEndOfList(tracker, current.sourceFile, compilerOptions.properties, newPluginsProp)
        } else if (ts.isArrayLiteralExpression(pluginsProperty.initializer)) {
          const pluginsArray = pluginsProperty.initializer

          const lspPluginElement = pluginsArray.elements.find((element) => {
            if (ts.isObjectLiteralExpression(element)) {
              const nameProperty = findPropertyInObject(element, "name")
              if (nameProperty && ts.isStringLiteral(nameProperty.initializer)) {
                return nameProperty.initializer.text === LSP_PLUGIN_NAME
              }
            }
            return false
          })

          if (!lspPluginElement) {
            descriptions.push(`Add ${LSP_PLUGIN_NAME} plugin to existing plugins array`)
            insertNodeAtEndOfList(tracker, current.sourceFile, pluginsArray.elements, pluginObject)
          } else if (ts.isObjectLiteralExpression(lspPluginElement)) {
            const diagnosticSeverityProperty = findPropertyInObject(lspPluginElement, "diagnosticSeverity")
            if (Option.isSome(target.diagnosticSeverities)) {
              const newDiagnosticSeverityValue = createDiagnosticSeverityObject(target.diagnosticSeverities.value)
              if (!diagnosticSeverityProperty) {
                descriptions.push(`Add diagnosticSeverity to ${LSP_PLUGIN_NAME} plugin`)
                insertNodeAtEndOfList(tracker, current.sourceFile, lspPluginElement.properties, ts.factory.createPropertyAssignment(
                  ts.factory.createStringLiteral("diagnosticSeverity"),
                  newDiagnosticSeverityValue
                ))
              } else if (ts.isPropertyAssignment(diagnosticSeverityProperty)) {
                descriptions.push(`Update diagnosticSeverity in ${LSP_PLUGIN_NAME} plugin`)
                tracker.replaceNode(current.sourceFile, diagnosticSeverityProperty.initializer, newDiagnosticSeverityValue)
              }
            } else if (diagnosticSeverityProperty) {
              descriptions.push(`Remove diagnosticSeverity from ${LSP_PLUGIN_NAME} plugin`)
              deleteNodeFromList(tracker, current.sourceFile, lspPluginElement.properties, diagnosticSeverityProperty)
            }
          }
        }
      }
    }
  )

  const fileChange = fileChanges.find((fc: ts.FileTextChanges) => fc.fileName === current.path)
  const changes = fileChange ? fileChange.textChanges : []

  if (changes.length === 0) {
    return { codeActions: [], messages }
  }

  return {
    codeActions: [{
      description: descriptions.join("; "),
      changes: [{
        fileName: current.sourceFile.fileName,
        textChanges: changes,
        isNewFile: false
      }]
    }],
    messages
  }
}

/**
 * Compute .vscode/settings.json changes using ChangeTracker
 */
const computeVSCodeSettingsChanges = (
  current: Assessment.VSCodeSettings,
  target: Target.VSCodeSettings
): ComputeFileChangesResult => {
  const descriptions: Array<string> = []
  const messages: Array<string> = []

  const rootObj = getRootObject(current.sourceFile)
  if (!rootObj) {
    return emptyFileChangesResult()
  }

  const ctx = createTrackerContext()

  const fileChanges = tsInternal.textChanges.ChangeTracker.with(
    ctx,
    (tracker: any) => {
      if (rootObj.properties.length === 0) {
        // Empty object — replace entirely
        const newProperties: Array<ts.PropertyAssignment> = []

        for (const [key, value] of Object.entries(target.settings)) {
          descriptions.push(`Add ${key} setting`)
          newProperties.push(
            ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral(key),
              typeof value === "string"
                ? ts.factory.createStringLiteral(value)
                : typeof value === "boolean"
                ? value ? ts.factory.createTrue() : ts.factory.createFalse()
                : ts.factory.createNull()
            )
          )
        }

        const newRootObj = ts.factory.createObjectLiteralExpression(newProperties, true)
        tracker.replaceNode(current.sourceFile, rootObj, newRootObj)
      } else {
        // Only add missing properties
        for (const [key, value] of Object.entries(target.settings)) {
          const existingProp = findPropertyInObject(rootObj, key)

          if (!existingProp) {
            descriptions.push(`Add ${key} setting`)

            const newProp = ts.factory.createPropertyAssignment(
              ts.factory.createStringLiteral(key),
              typeof value === "string"
                ? ts.factory.createStringLiteral(value)
                : typeof value === "boolean"
                ? value ? ts.factory.createTrue() : ts.factory.createFalse()
                : ts.factory.createNull()
            )
            insertNodeAtEndOfList(tracker, current.sourceFile, rootObj.properties, newProp)
          }
        }
      }
    }
  )

  const fileChange = fileChanges.find((fc: ts.FileTextChanges) => fc.fileName === current.path)
  const changes = fileChange ? fileChange.textChanges : []

  if (changes.length === 0) {
    return { codeActions: [], messages }
  }

  return {
    codeActions: [{
      description: descriptions.join("; "),
      changes: [{
        fileName: current.path,
        textChanges: changes,
        isNewFile: false
      }]
    }],
    messages
  }
}

/**
 * Compute the set of changes needed to go from assessment state to target state
 */
export const computeChanges = (
  assessment: Assessment.State,
  target: Target.State
): ComputeChangesResult => {
  let codeActions: ReadonlyArray<SetupCodeAction> = []
  let messages: ReadonlyArray<string> = []

  // Compute package.json changes
  const packageJsonResult = computePackageJsonChanges(assessment.packageJson, target.packageJson)
  codeActions = [...codeActions, ...packageJsonResult.codeActions]
  messages = [...messages, ...packageJsonResult.messages]

  // Compute tsconfig changes
  const tsconfigResult = computeTsConfigChanges(
    assessment.tsconfig,
    target.tsconfig,
    target.packageJson.lspVersion
  )
  codeActions = [...codeActions, ...tsconfigResult.codeActions]
  messages = [...messages, ...tsconfigResult.messages]

  // Compute VSCode settings changes if user selected VSCode editor
  if (target.editors.includes("vscode")) {
    if (Option.isSome(target.packageJson.lspVersion) && Option.isSome(target.vscodeSettings)) {
      const vscodeTarget = target.vscodeSettings.value

      if (Option.isSome(assessment.vscodeSettings)) {
        const vscodeResult = computeVSCodeSettingsChanges(assessment.vscodeSettings.value, vscodeTarget)
        codeActions = [...codeActions, ...vscodeResult.codeActions]
        messages = [...messages, ...vscodeResult.messages]
      } else {
        // File doesn't exist — emit a new-file code action with full content
        const dir = nodePath.dirname(assessment.packageJson.path)
        const vscodeSettingsPath = nodePath.join(dir, ".vscode", "settings.json")
        const content = JSON.stringify(vscodeTarget.settings, null, 2) + "\n"
        codeActions = [...codeActions, {
          description: "Create .vscode/settings.json",
          changes: [{
            fileName: vscodeSettingsPath,
            textChanges: [{ span: { start: 0, length: 0 }, newText: content }],
            isNewFile: true
          }]
        }]
      }
    }
  }

  // Add post-apply next-step messages
  if (Option.isSome(target.packageJson.lspVersion) && codeActions.length > 0) {
    messages = [
      ...messages,
      "Run `effect-tsgo patch` to complete the installation."
    ]
  } else if (Option.isNone(target.packageJson.lspVersion) && Option.isSome(assessment.packageJson.lspVersion)) {
    messages = [
      ...messages,
      "Run `effect-tsgo unpatch` to restore the original TypeScript-Go binary."
    ]
  }

  // Add editor-specific setup instructions as messages
  if (Option.isSome(target.packageJson.lspVersion) && target.editors.length > 0) {
    messages = [...messages, ""]

    if (target.editors.includes("vscode")) {
      messages = [
        ...messages,
        "VS Code / Cursor / VS Code-based editors:",
        "  1. Install the @typescript/native-preview extension",
        "  2. Open a TypeScript file and ensure the native TS server is active",
        "  3. The language service plugin will be loaded automatically",
        ""
      ]
    }

  }

  return { codeActions, messages }
}
