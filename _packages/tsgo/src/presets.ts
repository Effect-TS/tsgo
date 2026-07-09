import metadataJson from "./metadata.json" with { type: "json" }
import type { RuleSeverity } from "./setup/rule-info.js"

export interface DiagnosticPreset {
  readonly name: string
  readonly description: string
  readonly diagnosticSeverity: Readonly<Record<string, RuleSeverity>>
}

interface MetadataRule {
  readonly name: string
  readonly group: string
  readonly defaultSeverity: RuleSeverity
}

interface MetadataDocument {
  readonly rules: ReadonlyArray<MetadataRule>
  readonly presets: ReadonlyArray<DiagnosticPreset>
}

const metadata = metadataJson as MetadataDocument

const severityRank: Record<RuleSeverity, number> = {
  off: 0,
  suggestion: 1,
  message: 2,
  warning: 3,
  error: 4
}

const diagnosticDefaultSeverities: Readonly<Record<string, RuleSeverity>> = Object.fromEntries(
  metadata.rules.map((diagnostic) => [diagnostic.name, diagnostic.defaultSeverity])
)

const diagnosticNamesByLowerCase: Readonly<Record<string, string>> = Object.fromEntries(
  metadata.rules.map((diagnostic) => [diagnostic.name.toLowerCase(), diagnostic.name])
)

const diagnosticGroupsByName: Readonly<Record<string, string>> = Object.fromEntries(
  metadata.rules.map((diagnostic) => [diagnostic.name, diagnostic.group])
)

export const presets = metadata.presets as ReadonlyArray<DiagnosticPreset>

export type DiagnosticPresetName = (typeof presets)[number]["name"]

const presetsByName: Readonly<Record<DiagnosticPresetName, DiagnosticPreset>> = Object.fromEntries(
  presets.map((preset) => [preset.name, preset])
) as Record<DiagnosticPresetName, DiagnosticPreset>

export function compareRuleSeverity(left: RuleSeverity, right: RuleSeverity): number {
  return severityRank[left] - severityRank[right]
}

export function maxRuleSeverity(left: RuleSeverity, right: RuleSeverity): RuleSeverity {
  return compareRuleSeverity(left, right) >= 0 ? left : right
}

export function normalizeDiagnosticSeverities(
  severities: Readonly<Record<string, RuleSeverity>>
): Record<string, RuleSeverity> {
  const canonicalSeverities = Object.fromEntries(
    Object.entries(severities).map(([name, severity]) => [diagnosticNamesByLowerCase[name.toLowerCase()] ?? name, severity])
  )

  return Object.fromEntries(
    Object.entries(canonicalSeverities).flatMap(([name, severity]) => {
      const defaultSeverity = diagnosticDefaultSeverities[name]
      if (defaultSeverity !== undefined && defaultSeverity === severity) {
        return []
      }
      return [[name, severity]]
    })
  )
}

export function resolveDiagnosticSeverity(
  name: string,
  severities: Readonly<Record<string, RuleSeverity>>
): RuleSeverity {
  const canonicalName = diagnosticNamesByLowerCase[name.toLowerCase()] ?? name
  return severities[canonicalName] ?? severities[name] ?? diagnosticDefaultSeverities[canonicalName] ?? "off"
}

export function mergePresetDiagnosticSeverities(
  presetNames: ReadonlyArray<DiagnosticPresetName>
): Record<string, RuleSeverity> {
  const merged: Record<string, RuleSeverity> = {}

  for (const presetName of presetNames) {
    const preset = presetsByName[presetName]
    for (const [ruleName, severity] of Object.entries(preset.diagnosticSeverity)) {
      merged[ruleName] = ruleName in merged ? maxRuleSeverity(merged[ruleName]!, severity) : severity
    }
  }

  return merged
}

export function applyPresetDiagnosticSeverities(
  currentSeverities: Readonly<Record<string, RuleSeverity>>,
  presetNames: ReadonlyArray<DiagnosticPresetName>
): Record<string, RuleSeverity> {
  const mergedPresetSeverities = mergePresetDiagnosticSeverities(presetNames)
  const nextSeverities = normalizeDiagnosticSeverities(currentSeverities)

  for (const [ruleName, requiredSeverity] of Object.entries(mergedPresetSeverities)) {
    const currentSeverity = resolveDiagnosticSeverity(ruleName, nextSeverities)
    if (compareRuleSeverity(currentSeverity, requiredSeverity) < 0) {
      nextSeverities[ruleName] = requiredSeverity
    }
  }

  return normalizeDiagnosticSeverities(nextSeverities)
}

export function isPresetEnabled(
  presetName: DiagnosticPresetName,
  severities: Readonly<Record<string, RuleSeverity>>
): boolean {
  const preset = presetsByName[presetName]
  return Object.entries(preset.diagnosticSeverity).every(([ruleName, requiredSeverity]) =>
    compareRuleSeverity(resolveDiagnosticSeverity(ruleName, severities), requiredSeverity) >= 0
  )
}

export function getDiagnosticGroupPresetNames(group: string): ReadonlyArray<DiagnosticPresetName> {
  return presets
    .filter((preset) =>
      Object.keys(preset.diagnosticSeverity).every((ruleName) => diagnosticGroupsByName[ruleName] === group)
    )
    .map((preset) => preset.name)
}
