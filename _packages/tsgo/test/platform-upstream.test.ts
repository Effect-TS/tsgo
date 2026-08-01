import * as Effect from "effect/Effect"
import { describe, expect, it } from "vitest"
import { decodePackagedTypeScriptProfiles } from "../src/cli/platformUpstream.js"

describe("platform upstream metadata", () => {
  it("derives TypeScript binaries and prefers the latest binary", async() => {
    const profiles = await Effect.runPromise(decodePackagedTypeScriptProfiles(JSON.stringify({
      schemaVersion: 2,
      profiles: [
        {
          kind: "ts",
          name: "next",
          ts: { npmVersion: "7.1.0-dev", gitHead: "next-head" },
          binName: "tsc-next"
        },
        {
          kind: "ts",
          name: "latest",
          ts: { npmVersion: "7.0.0", gitHead: "latest-head" },
          binName: "tsc"
        },
        {
          kind: "oxlint",
          name: "oxlint",
          ts: { npmVersion: "7.0.0", gitHead: "latest-head" },
          tsgolint: { npmVersion: "1.0.0", gitHead: "tsgolint-head" },
          oxlint: { npmVersion: "1.0.0", gitHead: "oxlint-head" }
        }
      ]
    })))

    expect(profiles).toEqual([
      { binaryName: "tsc", tsVersion: "7.0.0", tsGitHead: "latest-head" },
      { binaryName: "tsc-next", tsVersion: "7.1.0-dev", tsGitHead: "next-head" }
    ])
  })

  it("rejects malformed manifests", async() => {
    await expect(Effect.runPromise(decodePackagedTypeScriptProfiles("{}"))).rejects.toThrow()
  })
})
