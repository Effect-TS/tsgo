use ets_go::{sprintf, sprintln};

use crate::helpers_test::*;
use crate::identity_forwarder_test::find_variable_initializer;

// go test: internal/typeparser/context_type_test.go:9 TestContextType_V4 531803b357665426
#[test]
fn test_context_type_v4() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintln!("Effect v4 not installed:", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v4_internal(
            t,
            r#"
import { Context, Effect, Layer } from "effect"

interface Config {
  readonly apiUrl: string
}
const Config = Context.Service<Config>("Config")

interface LocalContext<S> {
  readonly services: S
}

declare const wider: Context.Context<Config | "Other">
declare const untyped: any
declare const local: LocalContext<Config>

const made = Context.make(Config, { apiUrl: "https://api.example.com" })
const empty = Context.empty()
const widerRef = wider
export function generic<XR>(context: Context.Context<XR>) {
  const genericRef = context
  return genericRef
}

const anyRef = untyped
const effectRef = Effect.succeed(1)
const layerRef = Layer.empty
const localRef = local
"#,
        );
        let mut tp = compiled.type_parser();

        for name in ["made", "empty", "widerRef", "genericRef"] {
            if tp.context_type_at_location(find_variable_initializer(t, sf, name)).is_none() {
                t.error(sprintf!("expected %s to parse as a Context type", name));
            }
        }
        for name in ["anyRef", "effectRef", "layerRef", "localRef"] {
            if tp.context_type_at_location(find_variable_initializer(t, sf, name)).is_some() {
                t.error(sprintf!("expected %s not to parse as a Context type", name));
            }
        }
        if tp.context_type_at_location(None).is_some() {
            t.error("expected a nil node not to parse as a Context type");
        }
        if tp.context_type(None).is_some() {
            t.error("expected a nil type not to parse as a Context type");
        }
    });
}

// go test: internal/typeparser/context_type_test.go:64 TestContextType_V3 4396bf2c4c26adfc
#[test]
fn test_context_type_v3() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintln!("Effect v3 not installed:", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import { Context } from "effect"

class Config extends Context.Tag("Config")<Config, { readonly apiUrl: string }>() {}

const made = Context.make(Config, { apiUrl: "https://api.example.com" })
const empty = Context.empty()
"#,
        );
        let mut tp = compiled.type_parser();

        for name in ["made", "empty"] {
            if tp.context_type_at_location(find_variable_initializer(t, sf, name)).is_some() {
                t.error(sprintf!("expected v3 %s not to parse as a Context type", name));
            }
        }
    });
}
