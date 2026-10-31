// @effect-diagnostics *:off
// @effect-diagnostics emptySchemaStruct:warning
import { Schema } from "effect"
import { Tool as AiTool, Toolkit } from "effect/ai"
import { Tool } from "effect/ai"
import { make as makeTool } from "effect/ai/Tool"

// MUST TRIGGER (tool message, fix to Tool.EmptyParams): Schema.Struct({}) as Tool.make
// parameters, the documented replacement that omitted parameters default to
export const GetTime = Tool.make("GetTime", {
  parameters: Schema.Struct({}),
  success: Schema.String
})

// MUST TRIGGER (tool message, fix to Tool.EmptyParams): the parameters come first and the call is multi-line
export const GetDate = Tool.make("GetDate", {
  description: "Returns the current date",
  parameters: Schema.Struct({
    // no parameters yet
  }),
  success: Schema.String
})

// MUST TRIGGER (tool message, fix to Tool.EmptyParams): parenthesised parameters value
export const GetZone = Tool.make("GetZone", {
  parameters: (Schema.Struct({})),
  success: Schema.String
})

// MUST TRIGGER (tool message, fix to AiTool.EmptyParams): Tool imported under another name
export const GetUptime = AiTool.make("GetUptime", {
  parameters: Schema.Struct({}),
  success: Schema.Number
})

// MUST TRIGGER (tool message, fix to Tool.EmptyParams): Tool.dynamic parameters
export const DynamicTime = Tool.dynamic("DynamicTime", {
  parameters: Schema.Struct({}),
  success: Schema.String
})

// MUST TRIGGER (tool message, fix to Tool.EmptyParams): tool defined inline in a toolkit
export const Tools = Toolkit.make(
  Tool.make("GetVersion", {
    parameters: Schema.Struct({}),
    success: Schema.String
  })
)

// MUST TRIGGER (tool message, no fix): the callee is a named import, so there is no
// Tool namespace to build Tool.EmptyParams from
export const GetHost = makeTool("GetHost", {
  parameters: Schema.Struct({}),
  success: Schema.String
})

// MUST TRIGGER (general message, no fix): the struct is declared outside the tool, the
// parameters property only references it
const NoParams = Schema.Struct({})
export const GetLocale = Tool.make("GetLocale", {
  parameters: NoParams,
  success: Schema.String
})

// MUST TRIGGER (general message, no fix): an empty success schema is not a tool parameters position
export const Ack = Tool.make("Ack", {
  parameters: Schema.Struct({ id: Schema.String }),
  success: Schema.Struct({})
})

// MUST TRIGGER (general message, no fix): an empty struct nested inside non-empty parameters
export const Search = Tool.make("Search", {
  parameters: Schema.Struct({ query: Schema.String, filters: Schema.Struct({}) }),
  success: Schema.String
})

// MUST TRIGGER (general message, no fix): provider-defined tools are out of scope for Tool.EmptyParams
export const ProviderTool = Tool.providerDefined({
  id: "acme.time",
  customName: "AcmeTime",
  providerName: "time",
  parameters: Schema.Struct({}),
  success: Schema.String
})

// MUST TRIGGER (general message, no fix): a local object whose method is named like Tool.make
const LocalTool = {
  make: <A>(name: string, options: A) => ({ name, options })
}
export const localTool = LocalTool.make("Local", {
  parameters: Schema.Struct({})
})

// MUST NOT TRIGGER: the recommended empty parameters schema
export const GetTimeEmptyParams = Tool.make("GetTimeEmptyParams", {
  parameters: Tool.EmptyParams,
  success: Schema.String
})

// MUST NOT TRIGGER: omitted parameters default to Tool.EmptyParams
export const GetTimeDefault = Tool.make("GetTimeDefault", {
  success: Schema.String
})

// MUST NOT TRIGGER: non-empty parameters
export const GetWeather = Tool.make("GetWeather", {
  parameters: Schema.Struct({ city: Schema.String }),
  success: Schema.String
})
