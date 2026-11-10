package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// taggedDataTypeModules are the Effect modules whose data type is a union of
// tagged members, each with an is<Tag> guard, such as Option.isSome.
var taggedDataTypeModules = []struct {
	name       string
	descriptor packageSourceFileDescriptor
}{
	{"Option", effectOptionPackageSourceFileDescriptor},
	{"Exit", effectExitPackageSourceFileDescriptor},
	{"Result", effectResultPackageSourceFileDescriptor},
}

// TaggedDataTypeModule returns "Option", "Exit" or "Result" when that Effect
// module declares every member of the union t, and "" otherwise.
func (tp *TypeParser) TaggedDataTypeModule(t *checker.Type) string {
	if t == nil || t.Flags()&checker.TypeFlagsUnion == 0 {
		return ""
	}
	for _, module := range taggedDataTypeModules {
		declared := true
		for _, member := range t.Types() {
			if member.Symbol() == nil || !tp.symbolHasDeclarationInModuleCached(member.Symbol(), module.descriptor) {
				declared = false
				break
			}
		}
		if declared {
			return module.name
		}
	}
	return ""
}
