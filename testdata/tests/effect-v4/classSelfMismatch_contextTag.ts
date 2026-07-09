import { Context } from "effect"

interface ServiceShape {
  value: number
}

// valid usage: <ValidContextTag, ServiceShape> is correct because the Self type parameter is the same as the class name
export class ValidContextTag extends Context.Service<ValidContextTag, ServiceShape>()("ValidContextTag") {}

// valid usage: <InvalidContextTag, ServiceShape> is correct because the Self type parameter is the same as the class name
export class InvalidContextTag extends Context.Service<InvalidContextTag, ServiceShape>()("InvalidContextTag") {}
