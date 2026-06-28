import { describe, it, expect } from "vitest"
import {
  isNativeTypescriptVersion,
  nativeBackendTsdkPath,
  NATIVE_PREVIEW_PACKAGE_NAME,
  TYPESCRIPT_PACKAGE_NAME
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
  it("returns the node_modules folder for each backend package", () => {
    expect(nativeBackendTsdkPath(NATIVE_PREVIEW_PACKAGE_NAME)).toBe("node_modules/@typescript/native-preview")
    expect(nativeBackendTsdkPath(TYPESCRIPT_PACKAGE_NAME)).toBe("node_modules/typescript")
  })
})
