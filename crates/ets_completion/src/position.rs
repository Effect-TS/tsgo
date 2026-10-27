use ets_shim::ast::{self, Kind, Node, SourceFile};
use ets_shim::astnav;
use ets_shim::core::P;
use ets_shim::scanner;

// go: internal/completion/position.go:11 ExtendsClassCompletionData
// ExtendsClassCompletionData holds the parsed context for a completion
// triggered in the extends clause of a class declaration.
#[derive(Clone, Debug)]
pub struct ExtendsClassCompletionData {
    pub accessed_object: P<Node>,
    pub class_declaration: P<Node>,
    pub class_name: P<Node>,
    pub replacement_start: u32,
    pub replacement_length: u32,
}

// go: internal/completion/position.go:20 AccessedExpressionResult
// AccessedExpressionResult is the result from parsing an accessed expression at a cursor position.
#[derive(Clone, Debug)]
pub struct AccessedExpressionResult {
    pub accessed_object: P<Node>,
    pub outer_node: P<Node>,
    pub replacement_start: u32,
    pub replacement_length: u32,
}

// go: internal/completion/position.go:32 ParseAccessedExpressionForCompletion
// ParseAccessedExpressionForCompletion finds the accessed expression at the given cursor position.
// It handles three cases:
//   - extends Schema.Tag| — cursor after a property name in a PropertyAccessExpression
//   - extends Schema.| — cursor right after the dot
//   - extends Schema| — cursor after a standalone identifier
pub fn parse_accessed_expression_for_completion(
    sf: P<SourceFile>,
    position: u32,
) -> Option<AccessedExpressionResult> {
    let preceding_token = astnav::find_preceding_token(sf, position)?;

    let accessed_object: P<Node>;
    let outer_node: P<Node>;
    let replacement_start: u32;
    let replacement_length: u32;

    let parent = preceding_token.parent();
    let token_end = preceding_token.end().expect_source("a token of the file");
    if ast::is_identifier(preceding_token) && parent.is_some_and(ast::is_property_access_expression)
    {
        // extends Schema.Tag|
        let parent = parent.unwrap();
        let span_start = astnav::get_start_of_node(parent, sf, false);
        replacement_start = span_start;
        replacement_length = token_end - span_start;
        let prop = parent.as_property_access_expression();
        accessed_object = prop.expression();
        outer_node = parent;
    } else if ast::is_token_kind(preceding_token.kind())
        && preceding_token.kind() == Kind::DotToken
        && parent.is_some_and(ast::is_property_access_expression)
    {
        // extends Schema.|
        let parent = parent.unwrap();
        let span_start = astnav::get_start_of_node(parent, sf, false);
        replacement_start = span_start;
        replacement_length = token_end - span_start;
        let prop = parent.as_property_access_expression();
        accessed_object = prop.expression();
        outer_node = parent;
    } else if ast::is_identifier(preceding_token) && parent.is_some() {
        // extends Schema|
        let span_start = astnav::get_start_of_node(preceding_token, sf, false);
        replacement_start = span_start;
        replacement_length = token_end - span_start;
        accessed_object = preceding_token;
        outer_node = preceding_token;
    } else {
        return None;
    }

    // Skip if accessed object is inside an import declaration
    let import_decl = ast::find_ancestor(accessed_object, ast::is_import_declaration);
    if import_decl.is_some() {
        return None;
    }

    Some(AccessedExpressionResult {
        accessed_object,
        outer_node,
        replacement_start,
        replacement_length,
    })
}

// go: internal/completion/position.go:90 ParseDataForExtendsClassCompletion
// ParseDataForExtendsClassCompletion parses position context for a completion in the
// extends clause of a class declaration. It calls ParseAccessedExpressionForCompletion
// and then walks up the AST to find the enclosing ClassDeclaration.
pub fn parse_data_for_extends_class_completion(
    sf: P<SourceFile>,
    position: u32,
) -> Option<ExtendsClassCompletionData> {
    let result = parse_accessed_expression_for_completion(sf, position)?;

    // An identifier or a property access chain such as X.Schema, for files
    // that import the module through the package barrel.
    if !ast::is_entity_name_expression(result.accessed_object) {
        return None;
    }

    // Walk up from OuterNode.parent through ExpressionWithTypeArguments and HeritageClause
    let mut class_decl = result.outer_node.parent();
    while let Some(decl) = class_decl {
        if !(ast::is_expression_with_type_arguments(decl) || ast::is_heritage_clause(decl)) {
            break;
        }
        if decl.parent().is_none() {
            break;
        }
        class_decl = decl.parent();
    }

    let class_decl = class_decl.filter(|decl| ast::is_class_declaration(*decl))?;

    let class_name = class_decl.name()?;

    Some(ExtendsClassCompletionData {
        accessed_object: result.accessed_object,
        class_declaration: class_decl,
        class_name,
        replacement_start: result.replacement_start,
        replacement_length: result.replacement_length,
    })
}

impl ExtendsClassCompletionData {
    // go: internal/completion/position.go:131 ExtendsClassCompletionData.AccessedObjectText
    // AccessedObjectText returns the text of the accessed object node.
    pub fn accessed_object_text(&self) -> String {
        scanner::get_text_of_node(self.accessed_object)
    }

    // go: internal/completion/position.go:136 ExtendsClassCompletionData.ClassNameText
    // ClassNameText returns the text of the class name node.
    pub fn class_name_text(&self) -> String {
        scanner::get_text_of_node(self.class_name)
    }
}
