package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"testing"
)

func TestTransformationInputType(t *testing.T) {
	t.Parallel()
	subject, intermediate, output := &checker.Type{}, &checker.Type{}, &checker.Type{}
	flow := &PartialPipingFlow{Subject: PipingFlowSubject{OutType: subject}, Transformations: []PipingFlowTransformation{{OutType: intermediate}, {OutType: output}}}
	if flow.TransformationInputType(0) != subject {
		t.Fatal("first step must consume the subject type")
	}
	if flow.TransformationInputType(1) != intermediate {
		t.Fatal("later steps must consume the preceding output type")
	}
	for _, index := range []int{-1, 2} {
		if flow.TransformationInputType(index) != nil {
			t.Fatal("out-of-range step must have no input type")
		}
	}
	flow.Transformations[0].OutType = nil
	if flow.TransformationInputType(1) != nil {
		t.Fatal("missing output type must remain unknown")
	}
	if (*PipingFlow)(nil).TransformationInputType(0) != nil {
		t.Fatal("nil complete flow must have no input type")
	}
	if (*PartialPipingFlow)(nil).TransformationInputType(0) != nil {
		t.Fatal("nil flow must have no input type")
	}
}
