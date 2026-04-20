package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ServiceTypeId is the property key for the newer Context.Service variance struct.
const ServiceTypeId = "~effect/Context/Service"

// Service represents parsed v4 service type parameters.
type Service struct {
	Identifier *checker.Type // The service identifier/tag type
	Shape      *checker.Type // The service implementation shape
}

// parseServiceVarianceStruct extracts Identifier and Shape from a Service variance struct type.
func (tp *TypeParser) parseServiceVarianceStruct(t *checker.Type) *Service {
	identifier := tp.extractInvariantType(t, "_Identifier")
	if identifier == nil {
		return nil
	}

	shape := tp.extractInvariantType(t, "_Service")
	if shape == nil {
		return nil
	}

	return &Service{Identifier: identifier, Shape: shape}
}

// ServiceType parses a v4 service type and extracts Identifier, Shape parameters.
// Returns nil if the type is not a v4 service.
func (tp *TypeParser) ServiceType(t *checker.Type, atLocation *ast.Node) *Service {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.ServiceType, t, func() *Service {
		if tp.DetectEffectVersion() != EffectMajorV4 {
			return nil
		}
		if !tp.IsPipeableType(t, atLocation) {
			return nil
		}

		serviceKeyTypeIDType := tp.GetTypeOfPropertyByName(t, ServiceTypeId)
		if serviceKeyTypeIDType == nil {
			return nil
		}
		identifier := tp.GetTypeOfPropertyByName(t, "Identifier")
		if identifier == nil {
			return nil
		}
		shape := tp.GetTypeOfPropertyByName(t, "Service")
		if shape == nil {
			return nil
		}

		return &Service{Identifier: identifier, Shape: shape}
	})
}

// IsServiceType returns true if the type has the Service variance struct.
func (tp *TypeParser) IsServiceType(t *checker.Type, atLocation *ast.Node) bool {
	return tp.ServiceType(t, atLocation) != nil
}
