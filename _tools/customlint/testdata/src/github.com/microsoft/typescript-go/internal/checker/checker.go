package checker

type Node struct{}
type Checker struct{}

func (*Checker) GetSymbolAtLocation(n *Node) *Node { return n }
func (*Checker) GetTypeAtLocation(n *Node) *Node   { return n }

type ContextFlags int

func (*Checker) GetContextualType(n *Node, flags ContextFlags) *Node { return n }
