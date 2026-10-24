// @filename: api.ts
/** @stability unstable */
export const unstableApi = 1

// @filename: consumer.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning
import { unstableApi as renamed } from "./api"
import * as api from "./api"

export const namedUse = renamed
export const namespaceUse = api.unstableApi
