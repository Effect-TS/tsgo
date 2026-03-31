package typeparser

import (
	"github.com/microsoft/typescript-go/shim/checker"
)

// TypeParser groups checker-backed typeparser operations behind a shared
// checker/program pair so callers do not need to thread them through each call.
type TypeParser struct {
	program checker.Program
	checker *checker.Checker
}

// NewTypeParser builds a checker-backed TypeParser.
func NewTypeParser(p checker.Program, c *checker.Checker) *TypeParser {
	if p == nil {
		panic("typeparser.NewTypeParser: nil program")
	}
	if c == nil {
		panic("typeparser.NewTypeParser: nil checker")
	}
	return &TypeParser{program: p, checker: c}
}
