import rulesJson from "../rules.json" with { type: "json" }

export type RuleSeverity = "off" | "suggestion" | "message" | "warning" | "error"

export interface RuleInfo {
  readonly name: string
  readonly description: string
  readonly defaultSeverity: RuleSeverity
  readonly codes: ReadonlyArray<number>
}

export function getAllRules(): ReadonlyArray<RuleInfo> {
  return rulesJson as ReadonlyArray<RuleInfo>
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
