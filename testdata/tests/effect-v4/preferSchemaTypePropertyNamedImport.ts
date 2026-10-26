// @effect-diagnostics preferSchemaTypeProperty:error
import { Codec, Schema } from "effect/Schema"
import * as S from "effect/Schema"

export const User = S.Struct({ id: S.String })
export type UserType = Schema.Type<typeof User>
export type UserEncoded = Codec.Encoded<typeof User>
export type UserDecodingServices = Codec.DecodingServices<typeof User>
export type UserEncodingServices = Codec.EncodingServices<typeof User>
