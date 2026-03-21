import { copyFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

const packageDir = path.dirname(fileURLToPath(import.meta.url))
const schemaSourcePath = path.resolve(packageDir, "..", "..", "..", "schema.json")
const schemaTargetPath = path.resolve(packageDir, "..", "schema.json")

await copyFile(schemaSourcePath, schemaTargetPath)
