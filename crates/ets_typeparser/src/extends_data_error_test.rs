use std::collections::BTreeMap;

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::module_export_reference_test::kind_string;
use crate::*;

// go test: internal/typeparser/extends_data_error_test.go:10 TestExtendsDataError fc969481cc9448b4
#[test]
fn test_extends_data_error() {
    run(|t| {
        t.parallel();
        // Rust: Go's `bundledeffect.EnsurePackageInstalled` (not ported); mounting the profile reports the same
        // missing package.
        if let Err(err) = mount_effect(EFFECT_V3, &mut TestFS::default()) {
            t.skip(sprintf!("Effect v3 not installed: %v", err));
        }
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import { Data } from "effect"
interface Payload { name: string }
class Inline extends Data.Error<{ name: string }> {}
class Parenthesized extends (Data).Error<{ message: string }> {}
class NonNull extends Data!.Error<{ message: string }> {}
function getData() { return Data }
class Returned extends getData().Error<{ message: string }> {}
class Referenced extends Data.Error<Payload> {}
class Empty extends Data.Error {}
class Tagged extends Data.TaggedError("Tagged")<{ name: string }> {}
const UnrelatedData = { Error: class<Fields> { declare fields: Fields } }
function shadowedData() {
  const Data = UnrelatedData
  class Shadowed extends Data.Error<{ name: string }> {}
}
const EffectData = Data
function aliasedData() {
  const Data = EffectData
  class Aliased extends Data.Error<{ name: string }> {}
}
"#,
        );
        let mut tp = compiled.type_parser();
        let mut want: BTreeMap<String, Kind> = BTreeMap::from([
            ("Inline".to_string(), Kind::TypeLiteral),
            ("Parenthesized".to_string(), Kind::TypeLiteral),
            ("NonNull".to_string(), Kind::TypeLiteral),
            ("Returned".to_string(), Kind::TypeLiteral),
            ("Referenced".to_string(), Kind::TypeReference),
            ("Empty".to_string(), Kind::Unknown),
            ("Aliased".to_string(), Kind::TypeLiteral),
        ]);
        fn walk(
            t: &T,
            tp: &mut TypeParser<'_>,
            want: &mut BTreeMap<String, Kind>,
            node: P<Node>,
        ) -> bool {
            if node.kind() == Kind::ClassDeclaration {
                let name = node.name().map(|name| name.text().to_string()).unwrap_or_default();
                let result = tp.extends_data_error(node);
                if name == "Tagged" || name == "Shadowed" {
                    if result.is_some() {
                        t.fatal(sprintf!("class %s must not match Data.Error", name));
                    }
                } else {
                    let Some(result) =
                        result.filter(|r| r.class_name().is_some_and(|n| n.text() == name))
                    else {
                        t.fatal(sprintf!("expected Data.Error class %s", name));
                    };
                    let fields = result.fields_type_node();
                    let want_kind = want.get(&name).copied().unwrap_or(Kind::Unknown);
                    if name == "Empty" {
                        if fields.is_some() {
                            t.fatal("expected absent fields to remain nil");
                        }
                    } else if fields.is_none_or(|fields| fields.kind() != want_kind) {
                        t.fatal(sprintf!(
                            "expected %s payload kind %s",
                            name,
                            kind_string(want_kind)
                        ));
                    }
                    if tp.extends_data_tagged_error(node).is_some() {
                        t.fatal(sprintf!(
                            "Data.Error class %s must not match Data.TaggedError",
                            name
                        ));
                    }
                    want.remove(&name);
                }
            }
            node.for_each_child(&mut |child| walk(t, tp, want, child));
            false
        }
        walk(t, &mut tp, &mut want, sf.as_node());
        if !want.is_empty() {
            let want: BTreeMap<String, String> =
                want.into_iter().map(|(name, kind)| (name, kind_string(kind))).collect();
            t.fatal(sprintf!("missing classes: %v", want));
        }
    });
}
