// @effect-diagnostics *:off
// @effect-diagnostics schemaLiteralNonFinite:error

import { Schema } from "effect"

export const InvalidStatus = Schema.Literals([0, 1, NaN])
