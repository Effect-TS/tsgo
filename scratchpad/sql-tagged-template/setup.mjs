import { existsSync, lstatSync, mkdirSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const argument = process.argv[2]
if (!argument) throw new Error("Usage: node scratchpad/sql-tagged-template/setup.mjs /path/to/effect-checkout")
const effectRoot = resolve(argument)
const root = fileURLToPath(new URL("./", import.meta.url))
const packages = {
  effect: "packages/effect",
  "@effect/platform-node": "packages/platform/node",
  "@effect/sql-sqlite-node": "packages/sql/sqlite-node",
  "@types/node": "node_modules/@types/node"
}

for (const [name, path] of Object.entries(packages)) {
  const target = join(effectRoot, path)
  if (!existsSync(join(target, "package.json"))) throw new Error(`Missing ${target}; install the Effect checkout first`)
  const destination = join(root, "node_modules", name)
  mkdirSync(dirname(destination), { recursive: true })
  const previous = lstatSync(destination, { throwIfNoEntry: false })
  if (previous) {
    if (!previous.isSymbolicLink()) throw new Error(`Refusing to replace ${destination}`)
    unlinkSync(destination)
  }
  symlinkSync(target, destination, "dir")
}

writeFileSync(join(root, "tsconfig.json"), JSON.stringify({
  compilerOptions: {
    target: "ES2022",
    module: "NodeNext",
    strict: true,
    exactOptionalPropertyTypes: true,
    noEmit: true,
    allowImportingTsExtensions: true,
    resolveJsonModule: true,
    skipLibCheck: true,
    disableSourceOfProjectReferenceRedirect: true,
    types: ["node"],
    plugins: [{
      name: "@effect/language-service",
      diagnostics: false,
      experimentalLiteralTemplateStrings: true
    }]
  },
  include: ["*.ts"],
  references: Object.values(packages).slice(0, 3).map((path) => ({ path: join(effectRoot, path) }))
}, null, 2) + "\n")

console.log("Linked the Effect SQL spike. Build its referenced packages, then run ./tsgo -p scratchpad/sql-tagged-template.")
