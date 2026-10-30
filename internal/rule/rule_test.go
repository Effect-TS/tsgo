package rule

import "testing"

func TestByName(t *testing.T) {
	t.Parallel()
	rules := []Rule{{Name: "rule1"}, {Name: "rule2"}}

	if found := ByName(rules, "rule2"); found != &rules[1] {
		t.Errorf("expected pointer to rules[1], got %p", found)
	}
	if found := ByName(rules, "nonexistent"); found != nil {
		t.Errorf("expected nil for non-existent rule, got %+v", found)
	}
	if found := ByName(nil, "rule1"); found != nil {
		t.Errorf("expected nil for empty slice, got %+v", found)
	}
}
