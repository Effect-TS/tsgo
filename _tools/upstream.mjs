#!/usr/bin/env node

import { readFileSync, readdirSync, writeFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
export const upstreamPath = resolve(root, "_packages/tsgo/upstream.json")

const gitHeadPattern = /^[0-9a-f]{40}$/
const profileNames = ["next", "stable", "oxlint"]

const requireString = (value, path) => {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${path} must be a non-empty string`)
  }
  return value
}

const requireGitHead = (value, path) => {
  if (!gitHeadPattern.test(value ?? "")) {
    throw new Error(`${path} must be a 40-character lowercase Git SHA`)
  }
  return value
}

export const validateUpstream = (upstream) => {
  if (upstream?.schemaVersion !== 1) {
    throw new Error(`Unsupported upstream schema version: ${upstream?.schemaVersion}`)
  }

  for (const name of profileNames) {
    const profile = upstream[name]
    if (profile === undefined || profile === null || typeof profile !== "object") {
      throw new Error(`Missing upstream profile: ${name}`)
    }
    requireString(profile.tsVersion, `${name}.tsVersion`)
    requireGitHead(profile.tsGitHead, `${name}.tsGitHead`)
  }

  const oxlint = upstream.oxlint
  requireString(oxlint.tsgolintVersion, "oxlint.tsgolintVersion")
  requireGitHead(oxlint.tsgolintHead, "oxlint.tsgolintHead")
  requireString(oxlint.tsgolintRepository, "oxlint.tsgolintRepository")
  requireString(oxlint.oxlintVersion, "oxlint.oxlintVersion")
  requireGitHead(oxlint.oxlintHead, "oxlint.oxlintHead")
  requireString(oxlint.oxlintRepository, "oxlint.oxlintRepository")
  requireString(oxlint.oxlintTag, "oxlint.oxlintTag")

  return upstream
}

export const readUpstream = () => validateUpstream(JSON.parse(readFileSync(upstreamPath, "utf8")))

export const getProfile = (upstream, name) => {
  if (!profileNames.includes(name)) {
    throw new Error(`Unknown upstream profile: ${name}`)
  }
  return upstream[name]
}

const writeUpstream = (upstream) => {
  validateUpstream(upstream)
  writeFileSync(upstreamPath, `${JSON.stringify(upstream, null, 2)}\n`)
}

export const syncPackageMetadata = (upstream, packagesPath = resolve(root, "_packages")) => {
  const binaries = {
    tsc: {
      tsVersion: upstream.stable.tsVersion,
      tsGitHead: upstream.stable.tsGitHead,
    },
    "tsc-next": {
      tsVersion: upstream.next.tsVersion,
      tsGitHead: upstream.next.tsGitHead,
    },
  }

  for (const entry of readdirSync(packagesPath, { withFileTypes: true })) {
    if (!entry.isDirectory() || !entry.name.startsWith("tsgo-") || entry.name === "tsgo") {
      continue
    }
    const packageJsonPath = resolve(packagesPath, entry.name, "package.json")
    const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"))
    packageJson.effectTsgo = { binaries }
    writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`)
  }
}

const main = () => {
  const [command, ...args] = process.argv.slice(2)
  const upstream = readUpstream()

  switch (command) {
    case "profile": {
      const [name] = args
      process.stdout.write(JSON.stringify(getProfile(upstream, name)))
      return
    }
    case "field": {
      const [name, field] = args
      const value = getProfile(upstream, name)[field]
      requireString(value, `${name}.${field}`)
      process.stdout.write(value)
      return
    }
    case "set-typescript": {
      const [name, tsVersion, tsGitHead] = args
      const profile = getProfile(upstream, name)
      upstream[name] = { ...profile, tsVersion, tsGitHead }
      writeUpstream(upstream)
      return
    }
    case "sync-package-metadata":
      syncPackageMetadata(upstream)
      return
    case "validate":
      return
    default:
      throw new Error("Usage: upstream.mjs <validate|profile|field|set-typescript|sync-package-metadata> [...args]")
  }
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main()
  } catch (error) {
    console.error(`error: ${error.message}`)
    process.exitCode = 1
  }
}
