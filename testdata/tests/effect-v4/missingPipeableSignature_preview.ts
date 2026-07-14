// @effect-diagnostics *:off
// @effect-diagnostics missingPipeableSignature:warning

export const getAt = (self: ReadonlyArray<string>, index: number): string => self[index]
