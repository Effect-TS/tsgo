import {
  buildTargets,
  oxlintBindingName,
  oxlintBuildTargets,
  type BuildTarget,
  tsrsBuildTargets,
  type TsrsBuildTarget,
  type TsrsOptimization
} from "./build.ts"
import { tsrsBinaryFileName } from "./tsrsRelease.ts"
import type { ComponentName, Upstream } from "./upstream.ts"

export interface ReleaseArtifactPlan {
  readonly component: ComponentName
  readonly version: string
  readonly target: BuildTarget
  readonly runner: string
  readonly artifactName: string
  readonly fileName: string
  readonly destination: string
  // The job's timeout; the tsrs builds train PGO (and BOLT) profiles, as tsrs's release jobs (90 and 180 minutes) plus
  // the Effect workload.
  readonly timeoutMinutes: number
  // tsrs only: `repoctl build artifact --optimize`, the target's setting in tsrsBuildTargets.
  readonly optimize?: TsrsOptimization
}

const sortedVersions = (components: Readonly<Record<string, unknown>>) =>
  Object.keys(components).sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))

const artifactFileName = (component: ComponentName, target: BuildTarget) => {
  if (component === "oxlint") return oxlintBindingName(target as keyof typeof oxlintBuildTargets)
  if (component === "tsrs") return tsrsBinaryFileName(target as TsrsBuildTarget)
  const name = component === "typescript" ? "tsc" : "tsgolint"
  return target.startsWith("win32-") ? `${name}.exe` : name
}

const artifactPlan = (
  component: ComponentName,
  version: string,
  target: BuildTarget,
  runner: string,
  tsrs?: { readonly optimize: TsrsOptimization; readonly timeoutMinutes: number }
): ReleaseArtifactPlan => {
  const fileName = artifactFileName(component, target)
  return {
    component,
    version,
    target,
    runner,
    artifactName: `${target}__${component}__${version}`,
    fileName,
    destination: `_packages/tsgo-${target}/artifacts/${component}/${version}/${fileName}`,
    timeoutMinutes: tsrs?.timeoutMinutes ?? 45,
    ...(tsrs === undefined ? {} : { optimize: tsrs.optimize })
  }
}

const oxlintRunner = (target: BuildTarget) =>
  target.startsWith("darwin-") ? "macos-latest" : target.startsWith("win32-") ? "windows-latest" : "ubuntu-latest"

export const buildReleasePlan = (upstream: typeof Upstream.Type): ReadonlyArray<ReleaseArtifactPlan> => [
  ...sortedVersions(upstream.components.typescript).flatMap((version) =>
    (Object.keys(buildTargets) as Array<BuildTarget>).map((target) =>
      artifactPlan("typescript", version, target, "ubuntu-latest"))),
  ...sortedVersions(upstream.components["oxlint-tsgolint"]).flatMap((version) =>
    (Object.keys(oxlintBuildTargets) as Array<keyof typeof oxlintBuildTargets>).map((target) =>
      artifactPlan("oxlint-tsgolint", version, target, "ubuntu-latest"))),
  ...sortedVersions(upstream.components.oxlint).flatMap((version) =>
    (Object.keys(oxlintBuildTargets) as Array<keyof typeof oxlintBuildTargets>).map((target) =>
      artifactPlan("oxlint", version, target, oxlintRunner(target)))),
  // The patched tsrs binary, once per tsrs version and target, natively on the runner tsrs's own release uses.
  ...sortedVersions(upstream.components.tsrs ?? {}).flatMap((version) =>
    (Object.keys(tsrsBuildTargets) as Array<TsrsBuildTarget>).map((target) =>
      artifactPlan("tsrs", version, target, tsrsBuildTargets[target].runner, {
        optimize: tsrsBuildTargets[target].optimization,
        timeoutMinutes: target.startsWith("win32-") ? 180 : 120
      })))
]
