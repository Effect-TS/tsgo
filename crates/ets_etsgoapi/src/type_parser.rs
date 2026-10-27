use ets_shim::ast::Node;
use ets_shim::checker::{Checker, Type};
use ets_shim::compiler::Program;
use ets_shim::core::P;

// go: etsgoapi/type_parser.go:12 TypeParser 035a1ee1aed4eec0
// TypeParser wraps the internal type parser with a narrow public API.
//
// Rust: Go keeps the checker next to the inner parser; here the inner parser borrows it mutably, so the checker is
// `inner.checker()` (`TypeParser::checker`). Go's methods check for a nil receiver and a nil inner parser;
// neither can be nil here, so those guards are gone.
pub struct TypeParser<'c> {
    pub(crate) program: &'static Program,
    pub(crate) inner: ets_typeparser::TypeParser<'c>,
}

impl TypeParser<'_> {
    /// Rust-only: the checker the parser runs on (Go's callers hold it next to the parser).
    pub fn checker(&mut self) -> &mut Checker {
        self.inner.checker()
    }

    /// Rust-only: the program the parser was built for (Go's `program` field).
    pub fn program(&self) -> &'static Program {
        self.program
    }
}

// go: etsgoapi/type_parser.go:19 Effect ff6fe6de2dad8acd
// Effect represents parsed Effect<A, E, R> type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Effect {
    pub(crate) a: P<Type>,
    pub(crate) e: P<Type>,
    pub(crate) r: P<Type>,
}

impl Effect {
    // go: etsgoapi/type_parser.go:25 Effect.A 6ef90197255f6fc7
    pub fn a(&self) -> P<Type> {
        self.a
    }

    // go: etsgoapi/type_parser.go:27 Effect.E 190a2401a64c9acb
    pub fn e(&self) -> P<Type> {
        self.e
    }

    // go: etsgoapi/type_parser.go:29 Effect.R fad6c60fdffd2c4a
    pub fn r(&self) -> P<Type> {
        self.r
    }
}

// go: etsgoapi/type_parser.go:32 Layer cfdea3a3c413205a
// Layer represents parsed Layer<ROut, E, RIn> type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layer {
    pub(crate) r_out: P<Type>,
    pub(crate) e: P<Type>,
    pub(crate) r_in: P<Type>,
}

impl Layer {
    // go: etsgoapi/type_parser.go:38 Layer.ROut c71a15db01afbb53
    pub fn r_out(&self) -> P<Type> {
        self.r_out
    }

    // go: etsgoapi/type_parser.go:40 Layer.E ef30a4dbbb4ac9f5
    pub fn e(&self) -> P<Type> {
        self.e
    }

    // go: etsgoapi/type_parser.go:42 Layer.RIn 82e5776df34c707a
    pub fn r_in(&self) -> P<Type> {
        self.r_in
    }
}

// go: etsgoapi/type_parser.go:45 Service c504cb0085b45e7d
// Service represents parsed service identifier and implementation shape types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Service {
    pub(crate) identifier: P<Type>,
    pub(crate) shape: P<Type>,
}

impl Service {
    // go: etsgoapi/type_parser.go:50 Service.Identifier 9d97f0160b54ed99
    pub fn identifier(self) -> P<Type> {
        self.identifier
    }

    // go: etsgoapi/type_parser.go:52 Service.Shape 37ce50f4c447b1f1
    pub fn shape(self) -> P<Type> {
        self.shape
    }
}

// go: etsgoapi/type_parser.go:55 SchemaTypes c37c3c3c7f7dde49
// SchemaTypes represents parsed Schema type and encoded type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaTypes {
    pub(crate) a: P<Type>,
    pub(crate) e: P<Type>,
}

impl SchemaTypes {
    // go: etsgoapi/type_parser.go:60 SchemaTypes.A 5b0a10134d57f4f6
    pub fn a(self) -> P<Type> {
        self.a
    }

    // go: etsgoapi/type_parser.go:62 SchemaTypes.E 3c1b123807fd1e45
    pub fn e(self) -> P<Type> {
        self.e
    }
}

// go: etsgoapi/type_parser.go:65 NewTypeParser efd9f870333f3e96
// NewTypeParser builds a checker-backed TypeParser.
pub fn new_type_parser<'c>(program: &'static Program, checker: &'c mut Checker) -> TypeParser<'c> {
    TypeParser { program, inner: ets_typeparser::new_type_parser(program, checker) }
}

impl TypeParser<'_> {
    // go: etsgoapi/type_parser.go:70 TypeParser.GetTypeAtLocation 19922cd7b54159c4
    // GetTypeAtLocation returns the checker type for node using the parser's safety guards.
    pub fn get_type_at_location(&mut self, node: impl Into<Option<P<Node>>>) -> Option<P<Type>> {
        self.inner.get_type_at_location(node)
    }

    // go: etsgoapi/type_parser.go:78 TypeParser.EffectType 59394146b96be919
    // EffectType parses an Effect type and extracts A, E, R parameters.
    pub fn effect_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        effect_from_internal(self.inner.effect_type(t))
    }

    // go: etsgoapi/type_parser.go:86 TypeParser.LayerType b81e41d02086ff41
    // LayerType parses a Layer type and extracts ROut, E, RIn parameters.
    pub fn layer_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Layer> {
        layer_from_internal(self.inner.layer_type(t))
    }

    // go: etsgoapi/type_parser.go:94 TypeParser.ServiceType b3f1b95ae73ed139
    // ServiceType parses a v4 service type and extracts Identifier and Shape parameters.
    pub fn service_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Service> {
        service_from_internal(self.inner.service_type(t))
    }

    // go: etsgoapi/type_parser.go:102 TypeParser.ContextTag 0db7a3e31b7880bf
    // ContextTag parses a v3 Context.Tag type and extracts Identifier and Shape parameters.
    pub fn context_tag(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Service> {
        service_from_internal(self.inner.context_tag(t))
    }

    // go: etsgoapi/type_parser.go:110 TypeParser.EffectSchemaTypes a53f42d97e4ad5d8
    // EffectSchemaTypes extracts the type and encoded type from a Schema type.
    pub fn effect_schema_types(&mut self, t: impl Into<Option<P<Type>>>) -> Option<SchemaTypes> {
        schema_types_from_internal(self.inner.effect_schema_types(t))
    }

    // go: etsgoapi/type_parser.go:118 TypeParser.StreamType 4e3c6181a0adf20b
    // StreamType parses a Stream type and extracts A, E, R parameters.
    pub fn stream_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        effect_from_internal(self.inner.stream_type(t))
    }

    // go: etsgoapi/type_parser.go:127 TypeParser.UnrollUnionMembers 69a650e5b2773fcd
    // UnrollUnionMembers returns the constituent types of a union type,
    // or a single-element slice containing the type itself if it's not a union.
    //
    // Rust: Go's nil result (a nil type) is `None`.
    pub fn unroll_union_members(&self, t: impl Into<Option<P<Type>>>) -> Option<Vec<P<Type>>> {
        self.inner.unroll_union_members(t)
    }

    // go: etsgoapi/type_parser.go:135 TypeParser.IsYieldableErrorType a3f27b0d1c29b9db
    // IsYieldableErrorType reports whether the given type is assignable to Cause.YieldableError.
    pub fn is_yieldable_error_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.inner.is_yieldable_error_type(t)
    }
}

// go: etsgoapi/type_parser.go:142 effectFromInternal c60a68192a66ce88
pub(crate) fn effect_from_internal(effect: Option<ets_typeparser::Effect>) -> Option<Effect> {
    let effect = effect?;
    Some(Effect { a: effect.a(), e: effect.e(), r: effect.r() })
}

// go: etsgoapi/type_parser.go:149 layerFromInternal efc06fa94c06f14f
pub(crate) fn layer_from_internal(layer: Option<ets_typeparser::Layer>) -> Option<Layer> {
    let layer = layer?;
    Some(Layer { r_out: layer.r_out(), e: layer.e(), r_in: layer.r_in() })
}

// go: etsgoapi/type_parser.go:156 serviceFromInternal f06fd3a812a04535
pub(crate) fn service_from_internal(service: Option<ets_typeparser::Service>) -> Option<Service> {
    let service = service?;
    Some(Service { identifier: service.identifier(), shape: service.shape() })
}

// go: etsgoapi/type_parser.go:163 schemaTypesFromInternal b3aed39dc44de66b
pub(crate) fn schema_types_from_internal(
    schema_types: Option<ets_typeparser::SchemaTypes>,
) -> Option<SchemaTypes> {
    let schema_types = schema_types?;
    Some(SchemaTypes { a: schema_types.a(), e: schema_types.e() })
}
