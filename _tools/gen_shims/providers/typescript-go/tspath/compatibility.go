package tspath

import "github.com/microsoft/typescript-go/internal/tspath"

type PathKey = tspath.Path
type RootedFilePath = string
type RootedDirectoryPath = string
type RootedPath = string

func RootedFilePathFromAbsolute(path string) string      { return tspath.NormalizePath(path) }
func RootedDirectoryPathFromAbsolute(path string) string { return tspath.NormalizePath(path) }
func ToRootedFilePath(path, currentDirectory string) string {
	return tspath.GetNormalizedAbsolutePath(path, currentDirectory)
}
func ToRootedDirectoryPath(path, currentDirectory string) string {
	return tspath.GetNormalizedAbsolutePath(path, currentDirectory)
}

type CaseSensitivity uint8

const (
	CaseInsensitive CaseSensitivity = iota
	CaseSensitive
)

func ConvertToRelativePath(path, currentDirectory string, sensitivity CaseSensitivity) string {
	return tspath.ConvertToRelativePath(path, tspath.ComparePathsOptions{CurrentDirectory: currentDirectory, UseCaseSensitiveFileNames: sensitivity == CaseSensitive})
}

func UseCaseSensitiveFileNames(host interface{ UseCaseSensitiveFileNames() bool }) bool {
	return host.UseCaseSensitiveFileNames()
}

func PathKeyForFile(file string, caseSensitive bool) tspath.Path {
	return tspath.ToPath(file, "/", caseSensitive)
}
