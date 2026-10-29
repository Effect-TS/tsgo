package customlint

import (
	"testing"

	"golang.org/x/tools/go/analysis/analysistest"
)

func TestTypeAtLocation(t *testing.T) {
	dir, cleanup, err := analysistest.WriteFiles(map[string]string{
		"github.com/effect-ts/tsgo/internal/typeparser/parser.go": `package typeparser
type Node struct{}
type Type struct{ Flags int }
type Effect struct{}
type Layer struct{}
type TypeParser struct{}
func (*TypeParser) GetTypeAtLocation(n *Node) *Type { return nil }
func (*TypeParser) EffectType(t *Type) *Effect { return nil }
func (*TypeParser) LayerType(t *Type) *Layer { return nil }
func (*TypeParser) ArrayType(t *Type) *Type { return nil }
func (tp *TypeParser) inside(n *Node) *Effect { return tp.EffectType(tp.GetTypeAtLocation(n)) }`,
		"github.com/effect-ts/tsgo/internal/rules/rule.go": `package rules
import "github.com/effect-ts/tsgo/internal/typeparser"
type Other struct{}
func (Other) GetTypeAtLocation(n *typeparser.Node) *typeparser.Type { return nil }
func (Other) EffectType(t *typeparser.Type) *typeparser.Effect { return nil }
func uses(tp *typeparser.TypeParser, other Other, n *typeparser.Node, param *typeparser.Type, pick bool) {
	_ = tp.EffectType(tp.GetTypeAtLocation(n)) // want "use TypeParser.EffectTypeAtLocation instead of TypeParser.EffectType"
	_ = tp.EffectType((tp.GetTypeAtLocation(n))) // want "use TypeParser.EffectTypeAtLocation"
	_ = tp.LayerType(tp.GetTypeAtLocation(n)) // want "use TypeParser.LayerTypeAtLocation instead of TypeParser.LayerType"
	direct := tp.GetTypeAtLocation(n)
	_ = tp.EffectType(direct) // want "use TypeParser.EffectTypeAtLocation"
	_ = tp.LayerType(direct) // want "use TypeParser.LayerTypeAtLocation"
	var declared = tp.GetTypeAtLocation(n)
	_ = tp.EffectType(declared) // want "use TypeParser.EffectTypeAtLocation"
	var guarded *typeparser.Type
	if pick {
		guarded = tp.GetTypeAtLocation(n)
	}
	_ = tp.EffectType(guarded) // want "use TypeParser.EffectTypeAtLocation"
	mixed := tp.GetTypeAtLocation(n)
	if pick {
		mixed = param
	}
	_ = tp.EffectType(mixed)
	tuple, _ := tp.GetTypeAtLocation(n), 0
	_ = tp.EffectType(tuple) // want "use TypeParser.EffectTypeAtLocation"
	returned, _ := pair(tp, n)
	_ = tp.EffectType(returned)
	_ = tp.EffectType(param)
	checked := tp.GetTypeAtLocation(n)
	if checked == nil || (nil != checked) {
		return
	}
	_ = tp.EffectType((checked)) // want "use TypeParser.EffectTypeAtLocation"
	flagged := tp.GetTypeAtLocation(n)
	_ = tp.EffectType(flagged)
	_ = flagged.Flags
	passed := tp.GetTypeAtLocation(n)
	_ = tp.EffectType(passed)
	consume(passed)
	compared := tp.GetTypeAtLocation(n)
	_ = tp.EffectType(compared)
	_ = compared == param
	unsupported := tp.GetTypeAtLocation(n)
	_ = tp.EffectType(unsupported)
	_ = tp.ArrayType(unsupported)
	_ = tp.ArrayType(tp.GetTypeAtLocation(n))
	_ = tp.EffectType(other.GetTypeAtLocation(n))
	_ = other.EffectType(tp.GetTypeAtLocation(n))
}
func consume(*typeparser.Type) {}
func pair(tp *typeparser.TypeParser, n *typeparser.Node) (*typeparser.Type, bool) { return tp.GetTypeAtLocation(n), true }`,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer cleanup()
	analysistest.Run(t, dir, typeAtLocationAnalyzer, "github.com/effect-ts/tsgo/internal/...")
}
