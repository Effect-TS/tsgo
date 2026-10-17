package typeparser

import "testing"

func TestCheckedExpressionTypeRemembersOnlyCheckedSourceFiles(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/first.ts":  `export const first = { value: 1 }`,
		"/.src/second.ts": `export const second = { value: "second" }`,
	})
	defer done()

	firstFile, secondFile := files["/.src/first.ts"], files["/.src/second.ts"]
	first := findIdentifierByText(t, firstFile, "first", 0).Parent.Initializer()
	second := findIdentifierByText(t, secondFile, "second", 0).Parent.Initializer()

	if tp.checkedExpressionType(first) != nil || tp.checkedExpressionType(second) != nil {
		t.Fatal("expected no checked types before any check")
	}
	if tp.links.checkedExpressionTypeSourceFile != nil {
		t.Fatal("an unchecked source file was remembered")
	}

	c.GetDiagnostics(t.Context(), firstFile)
	firstType := tp.checkedExpressionType(first)
	if firstType == nil || c.TypeToString(firstType) != "{ value: number; }" {
		t.Fatalf("expected the checked type of first, got %v", firstType)
	}
	if tp.links.checkedExpressionTypeSourceFile != firstFile {
		t.Fatal("expected the checked first file to be remembered")
	}

	// a negative answer is not remembered, the file is asked again once checked
	if tp.checkedExpressionType(second) != nil {
		t.Fatal("expected no checked type for the unchecked second file")
	}
	if tp.links.checkedExpressionTypeSourceFile != firstFile {
		t.Fatal("an unchecked query replaced the remembered source file")
	}
	c.GetDiagnostics(t.Context(), secondFile)
	secondType := tp.checkedExpressionType(second)
	if secondType == nil || c.TypeToString(secondType) != "{ value: string; }" {
		t.Fatalf("expected the checked type of second, got %v", secondType)
	}
	if tp.links.checkedExpressionTypeSourceFile != secondFile {
		t.Fatal("expected the checked second file to be remembered")
	}

	if tp.checkedExpressionType(first) != firstType || tp.checkedExpressionType(second) != secondType {
		t.Fatal("expected the same checked types after switching files")
	}
}
