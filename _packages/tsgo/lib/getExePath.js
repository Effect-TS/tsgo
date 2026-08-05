import fs from "node:fs";
import module from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const typescriptPackageNames = ["typescript", "@typescript/native"];
function readJson(fileName) {
  return JSON.parse(fs.readFileSync(fileName, "utf8"));
}

function resolveTypeScriptPackage() {
  const cwdRequire = module.createRequire(path.join(process.cwd(), "noop.js"));

  for (const packageName of typescriptPackageNames) {
    let packageJson;
    try {
      packageJson = cwdRequire.resolve(packageName + "/package.json");
    }
    catch {
      continue;
    }

    const pkg = readJson(packageJson);
    const major = /^\s*(\d+)/.exec(pkg.version);
    if (major && Number(major[1]) >= 7) {
      if (typeof pkg.gitHead !== "string") {
        throw new Error("Missing gitHead in " + packageName + "/package.json.");
      }
      return pkg;
    }
  }

  throw new Error("Unable to resolve a native TypeScript package. Install typescript >= 7 or @typescript/native.");
}

export default function getExePath() {
  const __dirname = path.dirname(fileURLToPath(import.meta.url));
  const normalizedDirname = __dirname.replace(/\\/g, "/");

  if (normalizedDirname.endsWith("/_packages/tsgo/lib")) {
    let exe = path.resolve(__dirname, "..", "..", "..", "tsgo");
    if (process.platform === "win32") {
      exe += ".exe";
    }
    if (!fs.existsSync(exe)) {
      throw new Error("Executable not found: " + exe);
    }
    return exe;
  }

  const typescriptPackage = resolveTypeScriptPackage();
  const platformPackageName = "@effect/tsgo-" + process.platform + "-" + process.arch;
  let packageJson;

  try {
    const require_ = module.createRequire(path.join(__dirname, "getExePath.js"));
    packageJson = require_.resolve(platformPackageName + "/package.json");
  }
  catch {
    throw new Error("Unable to resolve " + platformPackageName + ". Either your platform is unsupported, or you are missing the package on disk.");
  }

  const packageDir = path.dirname(packageJson);
  const upstream = readJson(path.join(packageDir, "lib", "upstream.json"));
  if (upstream.schemaVersion !== 3 || typeof upstream.components?.typescript !== "object") {
    throw new Error("Invalid " + platformPackageName + "/lib/upstream.json.");
  }

  for (const [version, component] of Object.entries(upstream.components.typescript)) {
    if (typeof component?.gitHead !== "string") {
      throw new Error("Invalid TypeScript component metadata in " + platformPackageName + "/lib/upstream.json.");
    }
    let exe = path.join(packageDir, "artifacts", "typescript", version, "tsc");
    if (process.platform === "win32") {
      exe += ".exe";
      if (exe.length >= 248) {
        exe = "\\\\?\\" + exe;
      }
    }

    if (!fs.existsSync(exe)) {
      continue;
    }

    if (component.gitHead === typescriptPackage.gitHead) {
      return exe;
    }
  }

  throw new Error("No packaged Effect TypeScript executable matches installed TypeScript " + typescriptPackage.version + " gitHead " + typescriptPackage.gitHead + ".");
}
