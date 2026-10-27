use std::ops::BitOr;

use ets_readonly::Slice;
use ets_shim::ast::{Kind, ModifierFlags, Node, NodeList};
use ets_shim::core::P;

// go: internal/typeparser/lazy_expression.go:10 ParsedLazyExpression 211dad59d871b242
// ParsedLazyExpression represents a parsed arrow function or function expression
// with its inner expression extracted.
//
// Rust: the parameters are the function's own parameter list (`None` where Go's slice is nil).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParsedLazyExpression {
    pub(crate) node: P<Node>, // The original ArrowFunction or FunctionExpression node
    pub(crate) params: Option<&'static [P<Node>]>, // Parameter declarations (empty when parsed with thunk=true)
    pub(crate) body: P<Node>, // The function body as written (Expression or Block)
    pub(crate) expression: P<Node>, // The inner expression (return value)
}

impl ParsedLazyExpression {
    // go: internal/typeparser/lazy_expression.go:17 ParsedLazyExpression.Node 70e830b28ca10526
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/lazy_expression.go:19 ParsedLazyExpression.Params c8e594c485a9c7c6
    pub fn params(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.params)
    }

    // go: internal/typeparser/lazy_expression.go:21 ParsedLazyExpression.Body ac2eb0946fe9cca3
    pub fn body(&self) -> P<Node> {
        self.body
    }

    // go: internal/typeparser/lazy_expression.go:23 ParsedLazyExpression.Expression 2bae5d50bbce9c06
    pub fn expression(&self) -> P<Node> {
        self.expression
    }
}

// go: internal/typeparser/lazy_expression.go:26 LazyExpressionFlags edf1df552e7298c4
// LazyExpressionFlags controls which function shapes ParseLazyExpression accepts.
//
// Rust: a newtype over Go's `uint8` with the Go constants as associated consts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct LazyExpressionFlags(pub u8);

// go: internal/typeparser/lazy_expression.go:28 LazyExpressionNone ac54eb8f0d606ce9
impl LazyExpressionFlags {
    pub const None: LazyExpressionFlags = LazyExpressionFlags(0);
    // LazyExpressionThunk requires a function with no parameters.
    // Go: `1 << iota` with iota 1 on this line.
    pub const Thunk: LazyExpressionFlags = LazyExpressionFlags(1 << 1);
    // LazyExpressionAllowAsync additionally accepts async functions.
    pub const AllowAsync: LazyExpressionFlags = LazyExpressionFlags(1 << 2);
    // LazyExpressionAllowGenerator additionally accepts generator functions.
    pub const AllowGenerator: LazyExpressionFlags = LazyExpressionFlags(1 << 3);
    // LazyExpressionAllowMultiStatementBlock accepts statements before a sole final return.
    pub const AllowMultiStatementBlock: LazyExpressionFlags = LazyExpressionFlags(1 << 4);

    /// Rust-only: Go's `flags&mask != 0`.
    pub fn intersects(self, mask: LazyExpressionFlags) -> bool {
        self.0 & mask.0 != 0
    }
}

impl BitOr for LazyExpressionFlags {
    type Output = LazyExpressionFlags;
    fn bitor(self, rhs: LazyExpressionFlags) -> LazyExpressionFlags {
        LazyExpressionFlags(self.0 | rhs.0)
    }
}

// go: internal/typeparser/lazy_expression.go:43 ParseLazyExpression 291af64e6c142cb7
// ParseLazyExpression parses an arrow function or function expression, extracting its inner expression.
// By default, only synchronous, non-generator functions are accepted.
// Returns nil if the node is not a valid lazy expression.
pub fn parse_lazy_expression(
    node: impl Into<Option<P<Node>>>,
    flags: LazyExpressionFlags,
) -> Option<ParsedLazyExpression> {
    let node = ets_shim::ast::skip_parentheses(node.into()?);
    if !flags.intersects(LazyExpressionFlags::AllowAsync)
        && ets_shim::ast::get_combined_modifier_flags(node).intersects(ModifierFlags::Async)
    {
        return None;
    }

    let type_params: Option<P<NodeList>>;
    let params: Option<P<NodeList>>;
    let body: Option<P<Node>>;

    match node.kind() {
        Kind::ArrowFunction => {
            let f = node.as_arrow_function();
            type_params = f.type_parameters();
            params = f.parameters();
            body = f.body();
        }
        Kind::FunctionExpression => {
            let f = node.as_function_expression();
            if f.asterisk_token().is_some()
                && !flags.intersects(LazyExpressionFlags::AllowGenerator)
            {
                return None;
            }
            type_params = f.type_parameters();
            params = f.parameters();
            body = f.body();
        }
        _ => return None,
    }

    // Reject functions with type parameters
    if type_params.is_some_and(|type_params| !type_params.nodes().is_empty()) {
        return None;
    }

    // Thunks must not declare parameters.
    if flags.intersects(LazyExpressionFlags::Thunk)
        && params.is_some_and(|params| !params.nodes().is_empty())
    {
        return None;
    }

    let body = body?;

    // Build params list
    let param_nodes = params.map(|params| params.nodes());

    // Extract the inner expression
    let expr = if body.kind() == Kind::Block {
        let statements = body.as_block().statements().nodes();
        if statements.is_empty()
            || !flags.intersects(LazyExpressionFlags::AllowMultiStatementBlock)
                && statements.len() != 1
        {
            return None;
        }
        let stmt = statements[statements.len() - 1];
        if stmt.kind() != Kind::ReturnStatement {
            return None;
        }
        if flags.intersects(LazyExpressionFlags::AllowMultiStatementBlock)
            && ets_shim::ast::for_each_return_statement(body, |ret| ret != stmt)
        {
            return None;
        }
        stmt.as_return_statement().expression()?
    } else {
        // Expression body (arrow function only)
        body
    };

    Some(ParsedLazyExpression { node, params: param_nodes, body, expression: expr })
}
