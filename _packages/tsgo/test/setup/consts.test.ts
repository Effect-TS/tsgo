import { describe, it, expect } from "vitest"
import {
  defaultTypescriptPackageNames,
  isNativeTypescriptVersion,
  nativeBackendTsdkPath
} from "../../src/setup/consts.js"

describe("isNativeTypescriptVersion", () => {
  it("accepts typescript >= 7 exact versions, prereleases and ranges", () => {
    expect(isNativeTypescriptVersion("7.0.1-rc")).toBe(true)
    expect(isNativeTypescriptVersion("^7.0.1-rc")).toBe(true)
    expect(isNativeTypescriptVersion("^7")).toBe(true)
    expect(isNativeTypescriptVersion("~7.0.1")).toBe(true)
    expect(isNativeTypescriptVersion(">=7")).toBe(true)
    expect(isNativeTypescriptVersion("7.1.0")).toBe(true)
    expect(isNativeTypescriptVersion("10.0.0")).toBe(true)
  })

  it("rejects typescript < 7 (the JavaScript compiler)", () => {
    expect(isNativeTypescriptVersion("6.0.3")).toBe(false)
    expect(isNativeTypescriptVersion("^5.9.2")).toBe(false)
    expect(isNativeTypescriptVersion("4.9.5")).toBe(false)
  })

  it("rejects non-numeric / dist-tag specifiers", () => {
    expect(isNativeTypescriptVersion("latest")).toBe(false)
    expect(isNativeTypescriptVersion("rc")).toBe(false)
    expect(isNativeTypescriptVersion("")).toBe(false)
  })
})

describe("nativeBackendTsdkPath", () => {
  it("returns the node_modules folder for TypeScript", () => {
    expect(nativeBackendTsdkPath(defaultTypescriptPackageNames[0])).toBe("node_modules/typescript")
  })
})

describe("defaultTypescriptPackageNames", () => {
  it("tries typescript before the @typescript/native alias", () => {
    expect(defaultTypescriptPackageNames).toEqual(["typescript", "@typescript/native"])
  })
})
