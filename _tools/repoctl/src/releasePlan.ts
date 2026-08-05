import { buildTargets, oxlintBindingName, oxlintBuildTargets, type BuildTarget } from "./build.ts"
import type { ComponentName, Upstream } from "./upstream.ts"

export interface ReleaseArtifactPlan {
  readonly component: ComponentName
  readonly version: string
  readonly target: BuildTarget
  readonly runner: string
  readonly artifactName: string
  readonly fileName: string
  readonly destination: string
}

const sortedVersions = (components: Readonly<Record<string, unknown>>) =>
  Object.keys(components).sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))

const artifactFileName = (component: ComponentName, target: BuildTarget) => {
  if (component === "oxlint") return oxlintBindingName(target as keyof typeof oxlintBuildTargets)
  const name = component === "typescript" ? "tsc" : "tsgolint"
  return target.startsWith("win32-") ? `${name}.exe` : name
}

const artifactPlan = (
  component: ComponentName,
  version: string,
  target: BuildTarget,
  runner: string
): ReleaseArtifactPlan => {
  const fileName = artifactFileName(component, target)
  return {
    component,
    version,
    target,
    runner,
    artifactName: `${target}__${component}__${version}`,
    fileName,
    destination: `_packages/tsgo-${target}/artifacts/${component}/${version}/${fileName}`
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
      artifactPlan("oxlint", version, target, oxlintRunner(target))))
]
