package consumer

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	compiler "github.com/microsoft/TypeScript/tsc/shim/checker"
	legacy "github.com/microsoft/typescript-go/shim/checker"
)

type Alias = compiler.Checker
type PointerAlias = *Alias
type Promoted struct{ *Alias }
type Checker struct{}

func (*Checker) GetSymbolAtLocation(n *compiler.Node) *compiler.Node { return n }
func (*Checker) GetTypeAtLocation(n *compiler.Node) *compiler.Node   { return n }

func uses(c *compiler.Checker, old *legacy.Checker, alias PointerAlias, promoted *Promoted, n *compiler.Node) {
	c.GetSymbolAtLocation(n)                    // want "use TypeParser.GetSymbolAtLocation"
	c.GetTypeAtLocation(n)                      // want "use TypeParser.GetTypeAtLocation"
	old.GetSymbolAtLocation(nil)                // want "use TypeParser.GetSymbolAtLocation"
	old.GetTypeAtLocation(nil)                  // want "use TypeParser.GetTypeAtLocation"
	alias.GetSymbolAtLocation(n)                // want "use TypeParser.GetSymbolAtLocation"
	alias.GetTypeAtLocation(n)                  // want "use TypeParser.GetTypeAtLocation"
	promoted.GetSymbolAtLocation(n)             // want "use TypeParser.GetSymbolAtLocation"
	promoted.GetTypeAtLocation(n)               // want "use TypeParser.GetTypeAtLocation"
	_ = c.GetSymbolAtLocation                   // want "use TypeParser.GetSymbolAtLocation"
	_ = c.GetTypeAtLocation                     // want "use TypeParser.GetTypeAtLocation"
	_ = (*compiler.Checker).GetSymbolAtLocation // want "use TypeParser.GetSymbolAtLocation"
	_ = (*compiler.Checker).GetTypeAtLocation   // want "use TypeParser.GetTypeAtLocation"
	_ = (*Alias).GetSymbolAtLocation            // want "use TypeParser.GetSymbolAtLocation"
	_ = (*Alias).GetTypeAtLocation              // want "use TypeParser.GetTypeAtLocation"
	_ = PointerAlias.GetSymbolAtLocation        // want "use TypeParser.GetSymbolAtLocation"
	_ = PointerAlias.GetTypeAtLocation          // want "use TypeParser.GetTypeAtLocation"
	_ = promoted.GetSymbolAtLocation            // want "use TypeParser.GetSymbolAtLocation"
	_ = promoted.GetTypeAtLocation              // want "use TypeParser.GetTypeAtLocation"
	_ = (*Promoted).GetSymbolAtLocation         // want "use TypeParser.GetSymbolAtLocation"
	_ = (*Promoted).GetTypeAtLocation           // want "use TypeParser.GetTypeAtLocation"
	_ = old.GetSymbolAtLocation                 // want "use TypeParser.GetSymbolAtLocation"
	_ = old.GetTypeAtLocation                   // want "use TypeParser.GetTypeAtLocation"
	_ = (*legacy.Checker).GetSymbolAtLocation   // want "use TypeParser.GetSymbolAtLocation"
	_ = (*legacy.Checker).GetTypeAtLocation     // want "use TypeParser.GetTypeAtLocation"
	bound := c.GetSymbolAtLocation              // want "use TypeParser.GetSymbolAtLocation"
	boundType := c.GetTypeAtLocation            // want "use TypeParser.GetTypeAtLocation"
	bound(n)
	boundType(n)
	aliasedReceiver := c
	aliasedReceiver.GetSymbolAtLocation(n) // want "use TypeParser.GetSymbolAtLocation"
	aliasedReceiver.GetTypeAtLocation(n)   // want "use TypeParser.GetTypeAtLocation"
}

func allowed(p *typeparser.TypeParser, alias *typeparser.ParserAlias, lookalike *Checker, n *compiler.Node,
	dynamic interface {
		GetSymbolAtLocation(*compiler.Node) *compiler.Node
		GetTypeAtLocation(*compiler.Node) *compiler.Node
	}) {
	p.GetSymbolAtLocation(n)
	p.GetTypeAtLocation(n)
	alias.GetSymbolAtLocation(n)
	alias.GetTypeAtLocation(n)
	_ = p.GetSymbolAtLocation
	_ = p.GetTypeAtLocation
	_ = (*typeparser.TypeParser).GetSymbolAtLocation
	_ = (*typeparser.TypeParser).GetTypeAtLocation
	lookalike.GetSymbolAtLocation(n)
	lookalike.GetTypeAtLocation(n)
	dynamic.GetSymbolAtLocation(n)
	dynamic.GetTypeAtLocation(n)
}

func contextual(c *compiler.Checker, old *legacy.Checker, alias PointerAlias, promoted *Promoted, p *typeparser.TypeParser, n *compiler.Node) {
	c.GetContextualType(n, 0)                 // want "use TypeParser.GetContextualType"
	old.GetContextualType(nil, 0)             // want "use TypeParser.GetContextualType"
	alias.GetContextualType(n, 0)             // want "use TypeParser.GetContextualType"
	promoted.GetContextualType(n, 0)          // want "use TypeParser.GetContextualType"
	_ = c.GetContextualType                   // want "use TypeParser.GetContextualType"
	_ = (*compiler.Checker).GetContextualType // want "use TypeParser.GetContextualType"
	_ = old.GetContextualType                 // want "use TypeParser.GetContextualType"
	_ = (*legacy.Checker).GetContextualType   // want "use TypeParser.GetContextualType"
	_ = PointerAlias.GetContextualType        // want "use TypeParser.GetContextualType"
	_ = (*Promoted).GetContextualType         // want "use TypeParser.GetContextualType"
	p.GetContextualType(n, 0)
	_ = p.GetContextualType
}

func (*Checker) GetContextualType(n *compiler.Node, flags compiler.ContextFlags) *compiler.Node {
	return n
}

func allowedContextual(p *typeparser.ParserAlias, lookalike *Checker, n *compiler.Node,
	dynamic interface {
		GetContextualType(*compiler.Node, compiler.ContextFlags) *compiler.Node
	}) {
	p.GetContextualType(n, 0)
	lookalike.GetContextualType(n, 0)
	dynamic.GetContextualType(n, 0)
}
