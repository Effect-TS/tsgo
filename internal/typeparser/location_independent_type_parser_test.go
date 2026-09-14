package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

func TestEffectTypeParsesInstantiatedV3TypeWithoutLocation(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import * as Effect from "effect/Effect"

declare const effect: Effect.Effect<string, Error, number>
effect
`)
	defer done()

	effectType := tp.GetTypeAtLocation(findIdentifierByText(t, sf, "effect", 1))
	parsed := tp.EffectType(effectType)
	if parsed == nil {
		t.Fatal("expected instantiated v3 Effect type to parse without a location")
	}
	if got := c.TypeToString(parsed.A); got != "string" {
		t.Fatalf("expected success type string, got %s", got)
	}
	if got := c.TypeToString(parsed.E); got != "Error" {
		t.Fatalf("expected error type Error, got %s", got)
	}
	if got := c.TypeToString(parsed.R); got != "number" {
		t.Fatalf("expected requirements type number, got %s", got)
	}
}

func TestLayerTypeParsesInstantiatedV3TypeWithoutLocation(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import * as Layer from "effect/Layer"

declare const layer: Layer.Layer<string, Error, number>
layer
`)
	defer done()

	layerType := tp.GetTypeAtLocation(findIdentifierByText(t, sf, "layer", 1))
	parsed := tp.LayerType(layerType)
	if parsed == nil {
		t.Fatal("expected instantiated v3 Layer type to parse without a location")
	}
	if got := c.TypeToString(parsed.ROut); got != "string" {
		t.Fatalf("expected output requirements type string, got %s", got)
	}
	if got := c.TypeToString(parsed.E); got != "Error" {
		t.Fatalf("expected error type Error, got %s", got)
	}
	if got := c.TypeToString(parsed.RIn); got != "number" {
		t.Fatalf("expected input requirements type number, got %s", got)
	}
}

func TestEffectSchemaTypesParsesInstantiatedV3TypeWithoutLocation(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import * as Schema from "effect/Schema"

declare const schema: Schema.Schema<string, number>
schema
`)
	defer done()

	schemaType := tp.GetTypeAtLocation(findIdentifierByText(t, sf, "schema", 1))
	parsed := tp.EffectSchemaTypes(schemaType)
	if parsed == nil {
		t.Fatal("expected instantiated v3 Schema type to parse without a location")
	}
	if got := c.TypeToString(parsed.A); got != "string" {
		t.Fatalf("expected decoded type string, got %s", got)
	}
	if got := c.TypeToString(parsed.E); got != "number" {
		t.Fatalf("expected encoded type number, got %s", got)
	}
}

func TestStreamTypeParsesInstantiatedV3TypeWithoutLocation(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import * as Stream from "effect/Stream"

declare const stream: Stream.Stream<string, Error, number>
stream
`)
	defer done()

	streamType := tp.GetTypeAtLocation(findIdentifierByText(t, sf, "stream", 1))
	parsed := tp.StreamType(streamType)
	if parsed == nil {
		t.Fatal("expected instantiated v3 Stream type to parse without a location")
	}
	if got := c.TypeToString(parsed.A); got != "string" {
		t.Fatalf("expected element type string, got %s", got)
	}
	if got := c.TypeToString(parsed.E); got != "Error" {
		t.Fatalf("expected error type Error, got %s", got)
	}
	if got := c.TypeToString(parsed.R); got != "number" {
		t.Fatalf("expected requirements type number, got %s", got)
	}
}
