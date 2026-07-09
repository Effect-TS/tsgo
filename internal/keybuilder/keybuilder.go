package keybuilder

import (
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/tspath"
)

// Cyrb53 computes a fast non-cryptographic hash of the input string,
// producing a 16-character zero-padded hex string from two uint32 halves.
// This is a Go port of the cyrb53 function from the reference TS implementation.
func Cyrb53(str string) string {
	var h1 uint32 = 0xdeadbeef
	var h2 uint32 = 0x41c6ce57

	for i := range len(str) {
		ch := uint32(str[i])
		h1 = imul(h1^ch, 2654435761)
		h2 = imul(h2^ch, 1597334677)
	}

	h1 = imul(h1^(h1>>16), 2246822507)
	h1 ^= imul(h2^(h2>>13), 3266489909)
	h2 = imul(h2^(h2>>16), 2246822507)
	h2 ^= imul(h1^(h1>>13), 3266489909)

	return fmt.Sprintf("%08x%08x", h2, h1)
}

// imul emulates JavaScript's Math.imul: 32-bit integer multiplication
// that discards overflow (keeps only the lower 32 bits).
func imul(a, b uint32) uint32 {
	return uint32(uint64(a) * uint64(b))
}

// CreateString computes the expected key string for a class declaration.
// It takes the source file name, package name, package directory, class name,
// and target category, and returns the expected key string using the first
// matching key pattern. Returns empty string if no pattern matches or if
// package info is missing.
func CreateString(sourceFileName, packageName, packageDirectory, className, target string, keyPatterns []etscore.KeyPattern) string {
	if packageName == "" {
		return ""
	}

	for _, keyPattern := range keyPatterns {
		if keyPattern.Target != target {
			continue
		}

		// Construct onlyFileName: basename without extension, strip "index" suffix
		lastIndex := strings.LastIndex(sourceFileName, "/")
		var onlyFileName string
		if lastIndex == -1 {
			onlyFileName = sourceFileName
		} else {
			onlyFileName = sourceFileName[lastIndex+1:]
		}
		if lastExtIndex := strings.LastIndex(onlyFileName, "."); lastExtIndex != -1 {
			onlyFileName = onlyFileName[:lastExtIndex]
		}
		if strings.HasSuffix(strings.ToLower(onlyFileName), "/index") {
			onlyFileName = onlyFileName[:len(onlyFileName)-6]
		}
		// The TS reference strips "index" when the filename IS "index" (after removing extension)
		if strings.ToLower(onlyFileName) == "index" {
			onlyFileName = ""
		}
		onlyFileName = strings.TrimPrefix(onlyFileName, "/")

		// Construct subDirectory: directory relative to package directory
		subDirectory := tspath.GetDirectoryPath(sourceFileName)
		if !strings.HasPrefix(subDirectory, packageDirectory) {
			continue
		}
		subDirectory = subDirectory[len(packageDirectory):]
		if !strings.HasSuffix(subDirectory, "/") {
			subDirectory += "/"
		}
		subDirectory = strings.TrimPrefix(subDirectory, "/")
		for _, prefix := range keyPattern.SkipLeadingPath {
			if strings.HasPrefix(subDirectory, prefix) {
				subDirectory = subDirectory[len(prefix):]
				break
			}
		}

		// Construct parts based on pattern
		var parts []string
		classNameMatches := strings.EqualFold(onlyFileName, className)

		switch keyPattern.Pattern {
		case "package-identifier":
			parts = []string{packageName, onlyFileName}
			if !classNameMatches {
				parts = append(parts, className)
			}
		default: // "default" and "default-hashed"
			parts = []string{packageName, subDirectory, onlyFileName}
			if !classNameMatches {
				parts = append(parts, className)
			}
		}

		// Strip leading/trailing slashes from each part
		for i, part := range parts {
			part = strings.TrimPrefix(part, "/")
			part = strings.TrimSuffix(part, "/")
			parts[i] = part
		}

		// Filter empty parts and join
		var filtered []string
		for _, part := range parts {
			if strings.TrimSpace(part) != "" {
				filtered = append(filtered, part)
			}
		}
		fullKey := strings.Join(filtered, "/")

		// Hash if default-hashed pattern
		if keyPattern.Pattern == "default-hashed" {
			return Cyrb53(fullKey)
		}
		return fullKey
	}

	return ""
}
