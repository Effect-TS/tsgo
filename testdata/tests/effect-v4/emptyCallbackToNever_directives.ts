// @effect-diagnostics *:off
// @effect-diagnostics emptyCallbackToNever:error
import { Effect } from "effect"

// MUST TRIGGER: the file-level directive reports at error severity
export const fileSeverity = Effect.callback<never>(() => {})

// MUST NOT TRIGGER: the next-line directive disables this call only
// @effect-diagnostics-next-line emptyCallbackToNever:off
export const nextLineOff = Effect.callback<never>(() => {})

// MUST TRIGGER: the next-line directive does not reach this call
export const afterNextLine = Effect.callback<never>(() => {})

// @effect-diagnostics emptyCallbackToNever:suggestion
// MUST TRIGGER: a section directive lowers the severity to suggestion for the rest of the file
export const sectionSeverity = Effect.callback<never>(() => {})

// @effect-diagnostics emptyCallbackToNever:off
// MUST NOT TRIGGER: a section directive turns the rule off for the rest of the file
export const sectionOff = Effect.callback<never>(() => {})
