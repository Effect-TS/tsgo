package tspath

import "github.com/microsoft/TypeScript/tsc/internal/tspath"

// PathKeyForFile derives a canonical identity for an already rooted file.
func PathKeyForFile(file tspath.RootedFilePath, caseSensitive bool) tspath.PathKey {
	sensitivity := tspath.CaseInsensitive
	if caseSensitive {
		sensitivity = tspath.CaseSensitive
	}
	return sensitivity.PathKey(file.AsPath())
}

func UseCaseSensitiveFileNames(host interface{ CaseSensitivity() tspath.CaseSensitivity }) bool {
	return host.CaseSensitivity() == tspath.CaseSensitive
}
