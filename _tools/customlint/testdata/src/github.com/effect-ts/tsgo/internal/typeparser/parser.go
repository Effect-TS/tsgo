package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/checker"

type TypeParser struct{ checker *checker.Checker }
type ParserAlias = TypeParser

func (renamed *TypeParser) GetSymbolAtLocation(renamedInput *checker.Node) *checker.Node {
	if renamedInput == nil {
		return nil
	}
	return renamed.checker.GetSymbolAtLocation(renamedInput)
}

func (renamed TypeParser) GetTypeAtLocation(renamedInput *checker.Node) *checker.Node {
	return renamed.getTypeAtLocationUncached(renamedInput)
}

func (renamed TypeParser) getTypeAtLocationUncached(renamedInput *checker.Node) *checker.Node {
	if renamedInput == nil {
		return nil
	}
	renamed.checker.GetTypeAtLocation(renamedInput) // want "use TypeParser.GetTypeAtLocation"
	return renamed.checker.GetTypeAtLocation(renamedInput)
}

func (p *TypeParser) Other(n *checker.Node) *checker.Node {
	return p.checker.GetSymbolAtLocation(n) // want "use TypeParser.GetSymbolAtLocation"
}

func (p *TypeParser) OtherType(n *checker.Node) *checker.Node {
	return p.checker.GetTypeAtLocation(n) // want "use TypeParser.GetTypeAtLocation"
}

func GetSymbolAtLocation(c *checker.Checker, n *checker.Node) *checker.Node {
	return c.GetSymbolAtLocation(n) // want "use TypeParser.GetSymbolAtLocation"
}

func getTypeAtLocationUncached(c *checker.Checker, n *checker.Node) *checker.Node {
	return c.GetTypeAtLocation(n) // want "use TypeParser.GetTypeAtLocation"
}

type OtherParser struct{ checker *checker.Checker }

func (p *OtherParser) GetSymbolAtLocation(n *checker.Node) *checker.Node {
	return p.checker.GetSymbolAtLocation(n) // want "use TypeParser.GetSymbolAtLocation"
}

func (p OtherParser) getTypeAtLocationUncached(n *checker.Node) *checker.Node {
	return p.checker.GetTypeAtLocation(n) // want "use TypeParser.GetTypeAtLocation"
}
