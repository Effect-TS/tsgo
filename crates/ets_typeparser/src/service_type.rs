use ets_shim::ast::Node;
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/service_type.go:9 ServiceTypeId 2ddf4af254c2ea1e
// ServiceTypeId is the property key for the newer Context.Service variance struct.
pub const SERVICE_TYPE_ID: &str = "~effect/Context/Service";

// go: internal/typeparser/service_type.go:12 Service 47f4179f085b5256
// Service represents parsed v4 service type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Service {
    pub(crate) identifier: P<Type>, // The service identifier/tag type
    pub(crate) shape: P<Type>,      // The service implementation shape
}

impl Service {
    // go: internal/typeparser/service_type.go:17 Service.Identifier 9d97f0160b54ed99
    pub fn identifier(self) -> P<Type> {
        self.identifier
    }

    // go: internal/typeparser/service_type.go:19 Service.Shape 37ce50f4c447b1f1
    pub fn shape(self) -> P<Type> {
        self.shape
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/service_type.go:22 TypeParser.parseServiceVarianceStruct d2ed392470fcab5b
    // parseServiceVarianceStruct extracts Identifier and Shape from a Service variance struct type.
    pub(crate) fn parse_service_variance_struct(&mut self, t: Option<P<Type>>) -> Option<Service> {
        let identifier = self.extract_invariant_type(t, "_Identifier")?;

        let shape = self.extract_invariant_type(t, "_Service")?;

        Some(Service { identifier, shape })
    }

    // go: internal/typeparser/service_type.go:38 TypeParser.ServiceType e9f09273ed2280c3
    // ServiceType parses a v4 service type and extracts Identifier, Shape parameters.
    // Returns nil if the type is not a v4 service.
    pub fn service_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Service> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.service_type,
            t,
            |tp| {
                if tp.detect_effect_version() != EffectMajorVersion::V4 {
                    return None;
                }
                if !tp.is_pipeable_type(t) {
                    return None;
                }

                tp.get_type_of_property_by_name(t, SERVICE_TYPE_ID)?;
                let identifier = tp.get_type_of_property_by_name(t, "Identifier")?;
                let shape = tp.get_type_of_property_by_name(t, "Service")?;

                Some(Service { identifier, shape })
            },
        )
    }

    // go: internal/typeparser/service_type.go:71 TypeParser.ServiceTypeAtLocation 66a4f549b4302c0e
    // ServiceTypeAtLocation parses the type of node as a v4 service type.
    // Returns nil if the type is not a v4 service, without computing the type when
    // nodeCannotBeEffectLibraryType rules node out: a service carries the
    // "~effect/Context/Service" key, which such nodes never have.
    pub fn service_type_at_location(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<Service> {
        let t = self.effect_library_type_at_location(node);
        self.service_type(t)
    }
}
