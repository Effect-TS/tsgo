package typeparser

import (
	"fmt"

	"github.com/microsoft/typescript-go/shim/checker"
)

// UniqueTypesResult holds the result of AppendToUniqueTypesMap.
type UniqueTypesResult struct {
	AllIndexes []string // All type indexes encountered (both new and existing)
}

// AppendToUniqueTypesMap deduplicates types using bidirectional assignability checks.
// It unrolls union types, skips excluded types (via shouldExclude), and for each
// remaining type checks if it's already in the memory map. New types get fresh IDs
// ("t1", "t2", etc.); known types get their existing IDs.
// Returns all indexes encountered (both new and known).
func (tp *TypeParser) AppendToUniqueTypesMap(memory map[string]*checker.Type, initialType *checker.Type, shouldExclude func(*checker.Type) bool) UniqueTypesResult {
	c := tp.checker
	var allIndexes []string
	toTest := []*checker.Type{initialType}

	for len(toTest) > 0 {
		// Pop from the end of the slice
		last := len(toTest) - 1
		t := toTest[last]
		toTest = toTest[:last]

		if t == nil {
			continue
		}

		if shouldExclude != nil && shouldExclude(t) {
			continue
		}

		// If it's a union type, expand its members onto the worklist
		if t.Flags()&checker.TypeFlagsUnion != 0 {
			toTest = append(toTest, t.Types()...)
			continue
		}

		// Check if an equivalent type already exists in memory
		var matchedID string
		for typeID, knownType := range memory {
			if checker.Checker_isTypeAssignableTo(c, knownType, t) &&
				checker.Checker_isTypeAssignableTo(c, t, knownType) {
				matchedID = typeID
				break
			}
		}

		if matchedID == "" {
			// New type: assign a fresh ID
			newID := fmt.Sprintf("t%d", len(memory)+1)
			memory[newID] = t
			allIndexes = append(allIndexes, newID)
		} else {
			// Known type: record the existing ID
			allIndexes = append(allIndexes, matchedID)
		}
	}

	return UniqueTypesResult{AllIndexes: allIndexes}
}
