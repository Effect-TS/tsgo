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
func (tp *TypeParser) parseServiceVarianceStruct(t *checker.Type, atLocation *ast.Node) *Service {
	identifier := tp.extractInvariantType(t, atLocation, "_Identifier")
	if identifier == nil {
		return nil
	}

	shape := tp.extractInvariantType(t, atLocation, "_Service")
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

		serviceKeyTypeIDSymbol := tp.GetPropertyOfTypeByName(t, ServiceTypeId)
		if serviceKeyTypeIDSymbol == nil {
			return nil
		}
		identifierSymbol := tp.GetPropertyOfTypeByName(t, "Identifier")
		if identifierSymbol == nil {
			return nil
		}
		serviceSymbol := tp.GetPropertyOfTypeByName(t, "Service")
		if serviceSymbol == nil {
			return nil
		}

		identifier := tp.checker.GetTypeOfSymbolAtLocation(identifierSymbol, atLocation)
		shape := tp.checker.GetTypeOfSymbolAtLocation(serviceSymbol, atLocation)
		if identifier == nil || shape == nil {
			return nil
		}

		return &Service{Identifier: identifier, Shape: shape}
	})
}

// IsServiceType returns true if the type has the Service variance struct.
func (tp *TypeParser) IsServiceType(t *checker.Type, atLocation *ast.Node) bool {
	return tp.ServiceType(t, atLocation) != nil
}
