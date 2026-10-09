// @filename: /node_modules/jszip/package.json
{ "name": "jszip", "version": "3.10.1", "types": "index.d.ts" }

// @filename: /node_modules/jszip/index.d.ts
declare class JSZip {
  file(name: string): JSZip
}
export = JSZip

// @filename: /node_modules/simple-icons/package.json
{ "name": "simple-icons", "version": "15.0.0" }

// @filename: /node_modules/simple-icons/icons.json
[{ "title": "Effect" }]

// @filename: api.ts
/** @stability unstable */
export const unstableApi = 1

// @filename: consumer.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning
import { unstableApi } from "./api"

export const load = async () => {
  const JSZip = (await import("jszip")).default
  const { default: Zip } = await import("jszip")
  const icons = (await import("simple-icons/icons.json")).default
  return [new JSZip(), new Zip(), icons, unstableApi]
}
