# AST Navigation Utilities Reference

## Purpose

This document catalogues the AST traversal and node-finding utilities available through the shim layer. All Effect code must import these from the shim packages (`shim/astnav`, `shim/ast`), never from `typescript-go` directly.

## Token and Position Finding (`shim/astnav`)

These functions locate tokens or nodes at specific character positions within a source file.

| Function | Description |
|----------|-------------|
| **GetTokenAtPosition** | Returns the token at a position, including tokens in leading trivia. Creates synthesized tokens when needed and caches them. |
| **GetTouchingToken** | Returns the token touching a position, excluding leading trivia. |
| **GetTouchingPropertyName** | Gets the property name token at a position. Filters for property name literals, keywords, and private identifiers. |
| **FindPrecedingToken** | Finds the rightmost valid token that precedes or touches the given position. |
| **FindPrecedingTokenEx** | Extended version — allows starting from a specific node and optionally excluding JSDoc. |
| **FindNextToken** | Finds the next token after a given token within a parent node. |
| **FindChildOfKind** | Searches for a direct child node or token of a specified `ast.Kind` within a containing node. |
| **GetStartOfNode** | Gets the start position of a node, with option to include or exclude JSDoc. |
| **VisitEachChildAndJSDoc** | Visits each child of a node and its JSDoc comments with custom visitor callbacks. |

## Ancestor and Hierarchy Navigation (`shim/ast`)

These walk up the parent chain to find enclosing nodes that match a condition.

| Function | Description |
|----------|-------------|
| **FindAncestor** | Walks up the parent chain, returns the first ancestor where the callback returns `true`. |
| **FindAncestorKind** | Walks up the parent chain, returns the first ancestor matching a specific `ast.Kind`. |
| **FindAncestorOrQuit** | Like `FindAncestor` but the callback can return `Quit` to abort the search early (useful when you know the target can't be further up). |

## Container and Scope Finding (`shim/ast`)

These find the enclosing function, class, or `this`-binding container for a node.

| Function | Description |
|----------|-------------|
| **GetContainingFunction** | Finds the function declaration or expression containing the node. |
| **GetContainingClass** | Finds the class declaration containing the node. |
| **GetThisContainer** | Finds the container that defines the `this` binding. Options to include arrow functions and class computed property names. |
| **GetSourceFileOfNode** | Walks up to the root `SourceFile`. |

## Declaration and Binding Navigation (`shim/ast`)

These navigate declaration structures — bindings, destructuring patterns, and names.

| Function | Description |
|----------|-------------|
| **GetRootDeclaration** | For a `BindingElement`, traverses up through destructuring patterns to the root declaration. |
| **GetNameOfDeclaration** | Gets the name node of any declaration (variable, function, class, parameter, etc.). |
| **GetFirstIdentifier** | Recursively gets the leftmost identifier from a qualified name or property access chain (`a.b.c` → `a`). |
| **WalkUpBindingElementsAndPatterns** | Walks up binding elements and destructuring patterns to the containing declaration. |
| **FindConstructorDeclaration** | Finds the constructor declaration (with body) in a class. |

## Expression Walking (`shim/ast`)

These skip through wrapper nodes to reach the semantically meaningful expression or type.

| Function | Description |
|----------|-------------|
| **WalkUpParenthesizedExpressions** | Walks up through `(expr)` wrappers to the containing expression. |
| **WalkUpParenthesizedTypes** | Walks up through `(type)` wrappers to the containing type. |
| **ClimbPastPropertyAccess** | From a property name in `a.b`, climbs to the outer property access expression. |

## Call/Expression Target Checking (`shim/ast`)

These check whether a node occupies a specific syntactic role (e.g., is it the thing being called?).

| Function | Description |
|----------|-------------|
| **IsCallExpressionTarget** | Checks if a node is the target (callee) of a call expression. Options for element access and skipping outer expressions. |
| **IsNewExpressionTarget** | Checks if a node is the target of a `new` expression. |
| **IsCallOrNewExpressionTarget** | Combined check for call or new expression target. |
| **IsTaggedTemplateTag** | Checks if a node is the tag of a tagged template expression. |
| **IsDecoratorTarget** | Checks if a node is a decorator target. |
| **IsJsxOpeningLikeElementTagName** | Checks if a node is the tag name of a JSX opening element. |

## Traversal Iteration (`shim/ast`)

These iterate over children or specific node types within a subtree.

| Function | Description |
|----------|-------------|
| **ForEachChildAndJSDoc** | Iterates through each child of a node and its JSDoc comments using a `Visitor` callback. Return `true` to stop. |
| **ForEachReturnStatement** | Finds and iterates all return statements within a function body, traversing into control flow structures. |
| **ForEachDynamicImportOrRequireCall** | Iterates through all dynamic `import()` and `require()` calls in a source file. |

## Node Methods (on `*ast.Node`)

Every AST node exposes these traversal methods directly.

| Method | Description |
|--------|-------------|
| **ForEachChild(visitor)** | Iterates children with a `Visitor` callback. Return `true` to stop. |
| **VisitEachChild(visitor)** | Visits children with a `NodeVisitor`, allowing transformation. |
| **IterChildren()** | Returns a Go iterator (`iter.Seq[*ast.Node]`) over the node's children. |
| **Parent** | Field — the direct parent node (set during binding phase). |

## Safe Casting Pattern

When navigating AST nodes with `As*` methods, use a two-step guard pattern:

1. Check shape first with `ast.Is*` (or an equivalent explicit `Kind`/nil guard).
2. Cast second with `node.As*`.

Rules and analyzers must avoid chained unchecked casts (for example, calling `As*` and immediately dereferencing fields in the same expression). If any expected shape check fails, fail closed for that candidate (skip and continue traversal) rather than panicking.

## Visitor Patterns

Two visitor patterns are available:

- **`Visitor`** (`func(*ast.Node) bool`) — simple callback for read-only traversal. Return `true` to stop iteration.
- **`NodeVisitor`** — structured visitor with hooks (`VisitNode`, `VisitToken`, `VisitNodes`, `VisitModifiers`) for AST transformation. Created via `NewNodeVisitor`.

## Parent Binding

- **`SetParentInChildren(node)`** — recursively sets the `Parent` pointer on all descendants. Must be called after creating or modifying AST nodes to ensure the parent chain is valid. All ancestor/container-finding utilities depend on this.
