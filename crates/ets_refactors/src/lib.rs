mod async_await_to_fn;
mod async_await_to_fn_try_promise;
mod async_await_to_gen;
mod async_await_to_gen_try_promise;
mod effect_gen_to_fn;
mod function_to_arrow;
mod layer_magic;
mod make_schema_opaque;
mod make_schema_opaque_with_ns;
mod pipeable_to_datafirst;
mod refactors;
mod remove_unnecessary_effect_gen;
mod structural_type_to_schema;
mod toggle_lazy_const;
mod toggle_pipe_style;
mod toggle_return_type_annotation;
mod toggle_type_annotation;
mod type_to_effect_schema;
mod type_to_effect_schema_class;
mod wrap_with_effect_gen;
mod wrap_with_pipe;
mod write_tag_class_accessors;
pub use async_await_to_fn::*;
pub use async_await_to_fn_try_promise::*;
pub use async_await_to_gen::*;
pub use async_await_to_gen_try_promise::*;
pub use effect_gen_to_fn::*;
pub use function_to_arrow::*;
pub use layer_magic::*;
pub use make_schema_opaque::*;
pub use make_schema_opaque_with_ns::*;
pub use pipeable_to_datafirst::*;
pub use refactors::*;
pub use remove_unnecessary_effect_gen::*;
pub use structural_type_to_schema::*;
pub use toggle_lazy_const::*;
pub use toggle_pipe_style::*;
pub use toggle_return_type_annotation::*;
pub use toggle_type_annotation::*;
pub use type_to_effect_schema::*;
pub use type_to_effect_schema_class::*;
pub use wrap_with_effect_gen::*;
pub use wrap_with_pipe::*;
pub use write_tag_class_accessors::*;

/// Rust-only: Go's `ctx.Span.Pos()` (an `int`); a refactor span is a source range.
pub(crate) fn span_pos(ctx: &ets_refactor::Context<'_, '_>) -> u32 {
    ctx.span.pos().expect_source("a refactor span is a source range")
}
