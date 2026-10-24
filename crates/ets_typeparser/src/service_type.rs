use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/service_type.go:6 ServiceTypeId
// ServiceTypeId is the property key for the newer Context.Service variance struct.
pub const SERVICE_TYPE_ID: &str = "~effect/Context/Service";

// go: internal/typeparser/service_type.go:9 Service
// Service represents parsed v4 service type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Service {
    pub(crate) identifier: P<Type>, // The service identifier/tag type
    pub(crate) shape: P<Type>,      // The service implementation shape
}

impl Service {
    // go: internal/typeparser/service_type.go:14 Service.Identifier
    pub fn identifier(self) -> P<Type> {
        self.identifier
    }

    // go: internal/typeparser/service_type.go:16 Service.Shape
    pub fn shape(self) -> P<Type> {
        self.shape
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/service_type.go:19 TypeParser.parseServiceVarianceStruct
    // parseServiceVarianceStruct extracts Identifier and Shape from a Service variance struct type.
    pub(crate) fn parse_service_variance_struct(&mut self, t: Option<P<Type>>) -> Option<Service> {
        let identifier = self.extract_invariant_type(t, "_Identifier")?;

        let shape = self.extract_invariant_type(t, "_Service")?;

        Some(Service { identifier, shape })
    }

    // go: internal/typeparser/service_type.go:35 TypeParser.ServiceType
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

    // go: internal/typeparser/service_type.go:65 TypeParser.IsServiceType
    // IsServiceType returns true if the type has the Service variance struct.
    pub fn is_service_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.service_type(t).is_some()
    }
}
