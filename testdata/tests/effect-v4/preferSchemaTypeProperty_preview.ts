// @effect-diagnostics preferSchemaTypeProperty:error
import { Schema } from "effect"

const User = Schema.Struct({ name: Schema.String })
type User = Schema.Schema.Type<typeof User>
