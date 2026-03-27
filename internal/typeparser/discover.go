package typeparser

import "github.com/microsoft/typescript-go/shim/checker"

// DiscoveredPackage represents a package found in the program's source files.
type DiscoveredPackage struct {
	Name             string
	Version          *string
	DependsOnEffect  bool
	PackageDirectory string
}

// packageKey is used for deduplication of discovered packages.
type packageKey struct {
	name    string
	version string
	hasVer  bool
}

// EffectMajorVersion represents the detected major version of the Effect library.
type EffectMajorVersion int

const (
	EffectMajorUnknown EffectMajorVersion = 0
	EffectMajorV3      EffectMajorVersion = 3
	EffectMajorV4      EffectMajorVersion = 4
)

// String returns the string representation of the Effect major version.
func (v EffectMajorVersion) String() string {
	switch v {
	case EffectMajorV3:
		return "v3"
	case EffectMajorV4:
		return "v4"
	default:
		return "unknown"
	}
}

// DetectEffectVersion detects the major version of the Effect library from the program's
// source files. Returns EffectMajorUnknown if no Effect dependency is found or if
// conflicting versions are detected. The result is cached per checker lifetime.
func DetectEffectVersion(c *checker.Checker) EffectMajorVersion {
	if c == nil {
		return EffectMajorUnknown
	}
	links := GetEffectLinks(c)
	if links.detectEffectVersionComputed {
		return links.detectEffectVersionValue
	}
	result := detectEffectVersionUncached(c)
	links.detectEffectVersionValue = result
	links.detectEffectVersionComputed = true
	return result
}

// detectEffectVersionUncached performs the actual version detection logic.
func detectEffectVersionUncached(c *checker.Checker) EffectMajorVersion {
	packages := DiscoverPackages(c)

	var detected EffectMajorVersion
	found := false

	for _, pkg := range packages {
		if pkg.Name != "effect" {
			continue
		}

		var major EffectMajorVersion
		switch {
		case pkg.Version == nil:
			major = EffectMajorUnknown
		case len(*pkg.Version) > 0:
			switch (*pkg.Version)[0] {
			case '3':
				major = EffectMajorV3
			case '4':
				major = EffectMajorV4
			default:
				major = EffectMajorUnknown
			}
		default:
			major = EffectMajorUnknown
		}

		if !found {
			detected = major
			found = true
		} else if detected != major {
			return EffectMajorUnknown
		}
	}

	if !found {
		return EffectMajorUnknown
	}
	return detected
}

// SupportedEffectVersion returns the normalized supported Effect major version.
// It returns EffectMajorV4 when v4 is detected, and EffectMajorV3 for all other
// outcomes (including unknown). This is the central extension point for future
// compiler-option-based version forcing. The result is cached per checker lifetime.
func SupportedEffectVersion(c *checker.Checker) EffectMajorVersion {
	if c == nil {
		return EffectMajorV3
	}
	links := GetEffectLinks(c)
	if links.supportedEffectVersionComputed {
		return links.supportedEffectVersionValue
	}
	var result EffectMajorVersion
	if DetectEffectVersion(c) == EffectMajorV4 {
		result = EffectMajorV4
	} else {
		result = EffectMajorV3
	}
	links.supportedEffectVersionValue = result
	links.supportedEffectVersionComputed = true
	return result
}

// DetectEffectVersionString returns the exact version string of the Effect library.
// Returns "unknown" if no Effect dependency is found, if the version is nil, or if
// conflicting versions are detected.
func DetectEffectVersionString(c *checker.Checker) string {
	packages := DiscoverPackages(c)

	var detected string
	found := false

	for _, pkg := range packages {
		if pkg.Name != "effect" || pkg.Version == nil {
			continue
		}

		if !found {
			detected = *pkg.Version
			found = true
		} else if detected != *pkg.Version {
			return "unknown"
		}
	}

	if !found {
		return "unknown"
	}
	return detected
}

// DiscoverPackages iterates all source files in the program, resolves each one's
// nearest package.json, and returns a deduplicated list of discovered packages.
// Results are cached per checker so repeated calls within the same check cycle
// (from DetectEffectVersion, DetectEffectVersionString, duplicatePackage rule, etc.)
// do not re-scan all source files.
func DiscoverPackages(c *checker.Checker) []DiscoveredPackage {
	if c == nil {
		return nil
	}

	links := GetEffectLinks(c)
	if links.discoverPackagesComputed {
		return links.discoverPackagesValue
	}

	result := discoverPackagesUncached(c)
	links.discoverPackagesValue = result
	links.discoverPackagesComputed = true
	return result
}

// discoverPackagesUncached performs the actual source file scan.
func discoverPackagesUncached(c *checker.Checker) []DiscoveredPackage {
	prog, ok := c.Program().(sourceFileProgram)
	if !ok || prog == nil {
		return nil
	}

	pjProg, hasPjProg := c.Program().(packageJsonProgram)

	seen := make(map[packageKey]struct{})
	var result []DiscoveredPackage

	for _, sf := range prog.SourceFiles() {
		if sf == nil {
			continue
		}

		pkg := PackageJsonForSourceFile(c, sf)
		if pkg == nil {
			continue
		}

		name, ok := pkg.Name.GetValue()
		if !ok {
			continue
		}

		key := packageKey{name: name}
		var verPtr *string

		if ver, ok := pkg.Version.GetValue(); ok {
			key.version = ver
			key.hasVer = true
			verPtr = &ver
		}

		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}

		dependsOnEffect := false
		if peerDeps, ok := pkg.PeerDependencies.GetValue(); ok {
			_, dependsOnEffect = peerDeps["effect"]
		}

		var pkgDir string
		if hasPjProg {
			meta := pjProg.GetSourceFileMetaData(sf.Path())
			pkgDir = meta.PackageJsonDirectory
		}

		result = append(result, DiscoveredPackage{
			Name:             name,
			Version:          verPtr,
			DependsOnEffect:  dependsOnEffect,
			PackageDirectory: pkgDir,
		})
	}

	return result
}
