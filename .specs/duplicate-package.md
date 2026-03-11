# Duplicate Package Detection

## Goal

Warn users when multiple versions of the same Effect-related package are loaded into the program. This is a common source of cryptic type errors in monorepos and projects with complex lockfiles.

## Background

The `DiscoverPackages` utility already scans all program source files, reads their `package.json` scope, and returns a list of discovered packages with name, version, and whether they depend on `effect` as a peer dependency. The diagnostic builds on this existing infrastructure.

## Scope

- Implementing the `duplicatePackage` diagnostic rule.
- Extending `DiscoveredPackage` to include the on-disk package directory.
- Adding an `allowedDuplicatedPackages` plugin option for whitelisting known-safe duplicates.

## Requirements

### Detection

1. The rule runs once per source file (but the underlying package scan covers the entire program).
2. It calls `DiscoverPackages` to get all Effect-related packages in the program (packages where `name === "effect"` or where `effect` is a peer dependency).
3. For each package name with more than one distinct version, a warning is reported.
4. The warning is attached to the first statement of the source file (or the source file node itself if empty).
5. The warning message must list each conflicting version along with its resolved on-disk directory path, so the user can identify which installation is the culprit.
6. The message should suggest cleaning up the lockfile and mention the `allowedDuplicatedPackages` option as an escape hatch.

### Package Directory

7. `DiscoveredPackage` must be extended with a `PackageDirectory` field populated from the package.json info cache entry, so the diagnostic can report the on-disk path of each conflicting version.

### Plugin Option

8. An `allowedDuplicatedPackages` string list option must be added to the plugin options (default: empty).
9. If a package name appears in `allowedDuplicatedPackages`, the rule skips it even when multiple versions are present.

### Caching

10. The package scan result should be reused across source files within the same program check cycle. The rule must not re-scan all source files for every single file it visits.

## Non-Goals

- Detecting duplicate non-Effect packages (this rule is scoped to Effect ecosystem packages only).
- Auto-fixing lockfile conflicts (the diagnostic is informational only, no quick fix).
- Detecting version mismatches that don't involve actual duplicates (e.g., a single version that's outdated).

## Acceptance Criteria

1. When two versions of `effect` (or any Effect peer-dependent package) are present in the program, every source file shows a warning listing both versions and their paths.
2. Adding the package name to `allowedDuplicatedPackages` suppresses the warning for that package.
3. When only one version of each package is present, no warning is shown.
4. The diagnostic appears in the README diagnostic table with severity `warning` and V3+V4 support.
