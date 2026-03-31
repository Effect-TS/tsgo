package typeparser

import "github.com/microsoft/typescript-go/shim/checker"

// UnrollUnionMembers returns the constituent types of a union type,
// or a single-element slice containing the type itself if it's not a union.
func (tp *TypeParser) UnrollUnionMembers(t *checker.Type) []*checker.Type {
	if t == nil {
		return nil
	}
	if t.Flags()&checker.TypeFlagsUnion != 0 {
		return t.Types()
	}
	return []*checker.Type{t}
}
