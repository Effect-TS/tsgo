import * as Effect from "effect/Effect"
import { describe, expect, it } from "vitest"
import { decodePackagedTypeScriptProfiles } from "../src/cli/platformUpstream.js"

describe("platform upstream metadata", () => {
  it("derives TypeScript binaries and prefers the latest binary", async() => {
    const profiles = await Effect.runPromise(decodePackagedTypeScriptProfiles(JSON.stringify({
      schemaVersion: 3,
      typescript: { latest: "7.0.0", next: "7.1.0-dev" },
      components: {
        typescript: {
          "7.1.0-dev": { gitHead: "next-head" },
          "7.0.0": { gitHead: "latest-head" }
        }
      }
    })))

    expect(profiles).toEqual([
      {
        binaryName: "tsc",
        artifactPath: "artifacts/typescript/7.0.0/tsc",
        tsVersion: "7.0.0",
        tsGitHead: "latest-head"
      },
      {
        binaryName: "tsc-next",
        artifactPath: "artifacts/typescript/7.1.0-dev/tsc",
        tsVersion: "7.1.0-dev",
        tsGitHead: "next-head"
      }
    ])
  })

  it("rejects malformed manifests", async() => {
    await expect(Effect.runPromise(decodePackagedTypeScriptProfiles("{}"))).rejects.toThrow()
  })
})
