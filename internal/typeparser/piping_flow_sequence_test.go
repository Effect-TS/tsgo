package typeparser

import (
	"slices"
	"testing"
)

func TestFindTransformationSequences(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name           string
		steps, pattern []TransformationKind
		starts         []int
	}{
		{name: "nil flow"},
		{name: "empty flow", pattern: []TransformationKind{TransformationKindPipe}},
		{name: "empty pattern", steps: []TransformationKind{TransformationKindPipe}},
		{name: "longer than flow", steps: []TransformationKind{TransformationKindPipe}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindPipe}},
		{name: "nonconsecutive", steps: []TransformationKind{TransformationKindPipe, TransformationKindCall, TransformationKindDataFirst}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindDataFirst}},
		{name: "overlaps", steps: []TransformationKind{TransformationKindPipe, TransformationKindPipe, TransformationKindPipe}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindPipe}, starts: []int{0, 1}},
		{name: "multiple matches", steps: []TransformationKind{TransformationKindCall, TransformationKindPipe, TransformationKindDataFirst, TransformationKindCall, TransformationKindPipe, TransformationKindDataFirst}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindDataFirst}, starts: []int{1, 4}},
		{name: "whole flow", steps: []TransformationKind{TransformationKindPipe, TransformationKindCall}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindCall}, starts: []int{0}},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			var flow *PipingFlow
			if test.name != "nil flow" {
				flow = &PipingFlow{}
				for _, kind := range test.steps {
					flow.Transformations = append(flow.Transformations, PipingFlowTransformation{Kind: kind})
				}
			}
			var predicates []func(*PipingFlowTransformation) bool
			for _, kind := range test.pattern {
				predicates = append(predicates, func(step *PipingFlowTransformation) bool { return step.Kind == kind })
			}
			matches := flow.FindTransformationSequences(predicates...)
			var starts []int
			for _, match := range matches {
				starts = append(starts, match.Start)
				if len(match.Transformations) != len(test.pattern) {
					t.Fatal("match must contain exactly the requested steps")
				}
				for i := range match.Transformations {
					if &match.Transformations[i] != &flow.Transformations[match.Start+i] {
						t.Fatal("matched steps must refer to the original flow")
					}
				}
			}
			if !slices.Equal(starts, test.starts) {
				t.Fatalf("got starts %v, want %v", starts, test.starts)
			}
			if len(test.starts) == 0 && matches != nil {
				t.Fatal("no matches must return nil")
			}
		})
	}
	if (*PartialPipingFlow)(nil).FindTransformationSequences(func(*PipingFlowTransformation) bool { return true }) != nil {
		t.Fatal("nil partial flow must return nil")
	}
}
