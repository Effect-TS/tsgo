import metadataJson from "../metadata.json" with { type: "json" }

export type RuleSeverity = "off" | "suggestion" | "message" | "warning" | "error"

export interface DiagnosticPreview {
  readonly sourceText: string
  readonly diagnostics: ReadonlyArray<{ start: number; end: number; text: string }>
}

export interface RuleInfo {
  readonly name: string
  readonly group: string
  readonly description: string
  readonly defaultSeverity: RuleSeverity
  readonly fixable: boolean
  readonly supportedEffect: ReadonlyArray<string>
  readonly codes: ReadonlyArray<number>
  readonly preview: DiagnosticPreview
}

export interface GroupInfo {
  readonly id: string
  readonly name: string
  readonly description: string
}

export function getAllRules(): ReadonlyArray<RuleInfo> {
  return metadataJson.rules as ReadonlyArray<RuleInfo>
}

export function getAllGroups(): ReadonlyArray<GroupInfo> {
  return metadataJson.groups as ReadonlyArray<GroupInfo>
}

export function cycleSeverity(
  current: RuleSeverity,
  direction: "left" | "right"
): RuleSeverity {
  const order: ReadonlyArray<RuleSeverity> = ["off", "suggestion", "message", "warning", "error"]
  const currentIndex = order.indexOf(current)
  if (direction === "right") {
    return order[(currentIndex + 1) % order.length]
  }
  return order[(currentIndex - 1 + order.length) % order.length]
}

const shortNames: Record<RuleSeverity, string> = {
  off: "off",
  suggestion: "sugg",
  message: "info",
  warning: "warn",
  error: "err"
}

export const MAX_SEVERITY_LENGTH = Object.values(shortNames).reduce((max, name) => Math.max(max, name.length), 0)

export function getSeverityShortName(severity: RuleSeverity): string {
  return shortNames[severity] ?? "???"
}
