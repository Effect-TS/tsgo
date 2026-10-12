package checker

type Node struct{}
type Checker struct{}

func (*Checker) GetSymbolAtLocation(n *Node) *Node { return n }
func (*Checker) GetTypeAtLocation(n *Node) *Node   { return n }
