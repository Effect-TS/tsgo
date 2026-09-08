package typeparser

import "testing"

func TestPipingFlowMatchesShape(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name           string
		subjectMatches bool
		steps, pattern []TransformationKind
		prefix, exact  bool
	}{
		{name: "subject only", subjectMatches: true, prefix: true, exact: true},
		{name: "subject mismatch"},
		{name: "subject only with extra steps", subjectMatches: true, steps: []TransformationKind{TransformationKindPipe}, prefix: true},
		{name: "whole flow", subjectMatches: true, steps: []TransformationKind{TransformationKindPipe, TransformationKindCall}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindCall}, prefix: true, exact: true},
		{name: "prefix with extra steps", subjectMatches: true, steps: []TransformationKind{TransformationKindPipe, TransformationKindCall}, pattern: []TransformationKind{TransformationKindPipe}, prefix: true},
		{name: "too few steps", subjectMatches: true, steps: []TransformationKind{TransformationKindPipe}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindCall}},
		{name: "empty flow with expected step", subjectMatches: true, pattern: []TransformationKind{TransformationKindPipe}},
		{name: "later step mismatch", subjectMatches: true, steps: []TransformationKind{TransformationKindPipe, TransformationKindCall}, pattern: []TransformationKind{TransformationKindPipe, TransformationKindPipe}},
		{name: "matching sequence is not at head", subjectMatches: true, steps: []TransformationKind{TransformationKindCall, TransformationKindPipe}, pattern: []TransformationKind{TransformationKindPipe}},
		{name: "steps match but subject does not", steps: []TransformationKind{TransformationKindPipe}, pattern: []TransformationKind{TransformationKindPipe}},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			flow := &PipingFlow{}
			for _, kind := range test.steps {
				flow.Transformations = append(flow.Transformations, PipingFlowTransformation{Kind: kind})
			}
			subject := func(actual *PipingFlowSubject) bool {
				if actual != &flow.Subject {
					t.Fatal("subject predicate must receive the original subject")
				}
				return test.subjectMatches
			}
			var predicates []func(*PipingFlowTransformation) bool
			for i, kind := range test.pattern {
				predicates = append(predicates, func(step *PipingFlowTransformation) bool {
					if step != &flow.Transformations[i] {
						t.Fatal("step predicate must receive the corresponding original transformation")
					}
					return step.Kind == kind
				})
			}
			if got := flow.MatchesPrefix(subject, predicates...); got != test.prefix {
				t.Fatalf("MatchesPrefix = %v, want %v", got, test.prefix)
			}
			if got := flow.MatchesExactly(subject, predicates...); got != test.exact {
				t.Fatalf("MatchesExactly = %v, want %v", got, test.exact)
			}
		})
	}
}

func TestPipingFlowMatchesShapeNil(t *testing.T) {
	t.Parallel()
	subject := func(*PipingFlowSubject) bool { t.Fatal("nil flow must not evaluate predicates"); return true }
	var partial *PartialPipingFlow
	var complete *PipingFlow
	if partial.MatchesPrefix(subject) || partial.MatchesExactly(subject) || complete.MatchesPrefix(subject) || complete.MatchesExactly(subject) {
		t.Fatal("nil flows must not match")
	}
}
