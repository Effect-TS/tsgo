package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseConfigSortsAndValidates(t *testing.T) {
	config, err := parseConfig([]byte(`{"crates":["tsrs_core","tsrs_ast"],"items":["tsrs_core::P"]}`))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Join(config.Crates, ",") != "tsrs_ast,tsrs_core" {
		t.Fatalf("crates = %v", config.Crates)
	}
	for _, invalid := range []string{
		`{"crates":[]}`,
		`{"crates":["serde"]}`,
		`{"crates":["tsrs_ast","tsrs_ast"]}`,
		`{"crates":["tsrs_ast"],"items":["tsrs_core::P"]}`,
		`{"crates":["tsrs_ast"],"items":["tsrs_ast"]}`,
		`{"crates":["tsrs_ast"],"unknown":true}`,
	} {
		if _, err := parseConfig([]byte(invalid)); err == nil {
			t.Errorf("parseConfig(%s) succeeded", invalid)
		}
	}
}

func TestRender(t *testing.T) {
	config := Config{Crates: []string{"tsrs_ast", "tsrs_core"}, Items: []string{"tsrs_core::P"}}
	lib := renderLib(config)
	for _, want := range []string{"pub use tsrs_ast as ast;\n", "pub use tsrs_core as core;\n", "pub use tsrs_core::P;\n", "DO NOT EDIT"} {
		if !strings.Contains(lib, want) {
			t.Errorf("lib.rs lacks %q:\n%s", want, lib)
		}
	}
	cargo := renderCargoToml(config, "../../tsrs")
	if !strings.Contains(cargo, `tsrs_ast = { path = "../../tsrs/crates/tsrs_ast" }`) {
		t.Errorf("Cargo.toml lacks the tsrs_ast path dependency:\n%s", cargo)
	}
}

func TestGenerate(t *testing.T) {
	root := t.TempDir()
	tsrs := filepath.Join(root, "tsrs")
	for _, crate := range []string{"tsrs_ast", "tsrs_core"} {
		dir := filepath.Join(tsrs, "crates", crate)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[package]\nname = \""+crate+"\"\n"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	configPath := filepath.Join(root, "config.json")
	if err := os.WriteFile(configPath, []byte(`{"crates":["tsrs_ast","tsrs_core"],"items":[]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	stale := filepath.Join(root, "crates", "ets_shim", "src", "stale.rs")
	if err := os.MkdirAll(filepath.Dir(stale), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(stale, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := generate(root, tsrs, configPath); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(stale); !os.IsNotExist(err) {
		t.Errorf("stale file survived: %v", err)
	}
	cargo, err := os.ReadFile(filepath.Join(root, "crates", "ets_shim", "Cargo.toml"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(cargo), `path = "../../tsrs/crates/tsrs_core"`) {
		t.Errorf("Cargo.toml:\n%s", cargo)
	}

	if err := os.WriteFile(configPath, []byte(`{"crates":["tsrs_missing"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := generate(root, tsrs, configPath); err == nil {
		t.Error("generate succeeded with a crate missing from the checkout")
	}
}
