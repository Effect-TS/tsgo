package rulerunner_test

import (
	"strings"
	"testing"
)

func TestApiStabilityLeakInternalProperties(t *testing.T) {
	t.Parallel()
	prefix := "/** @stability experimental */\ninterface E { value: string }\n"
	tests := []struct {
		name   string
		source string
		want   bool
	}{
		{
			name: "interface property",
			source: prefix + `export interface Api {
/** @internal */ hidden?: E
visible?: string
}`,
		},
		{
			name: "tagged internal property",
			source: prefix + `export interface Api {
/** @internal
 * @stability experimental */ hidden: E
}`,
		},
		{
			name: "public peer stays checked",
			source: prefix + `export interface Api {
/** @internal */ hidden?: E
visible?: E
}`,
			want: true,
		},
		{
			name:   "internal referenced type stays checked",
			source: "/** @internal\n * @stability experimental */\ninterface E { value: string }\nexport interface Api { visible: E }",
			want:   true,
		},
		{
			name: "class instance and static properties",
			source: prefix + `export class Api {
/** @internal */ hidden!: E
/** @internal */ static hidden?: E
}`,
		},
		{
			name: "internal method and accessor",
			source: prefix + `export class Api {
/** @internal */ hidden(): E { return null as unknown as E }
/** @internal */ get value(): E { return null as unknown as E }
}`,
		},
		{
			name: "inherited internal property",
			source: prefix + `interface Base {
/** @internal */ hidden?: E
}
export interface Api extends Base {}`,
		},
		{
			name: "generic internal property",
			source: prefix + `interface Generic<T> {
/** @internal */ hidden?: T
}
export declare const Api: Generic<E>`,
			// A represented generic argument is itself part of the public API,
			// even when a property using that argument is internal.
			want: true,
		},
		{
			name: "internal anonymous property with erased alias",
			source: `/** @stability experimental */
type E = string
export type Api = {
/** @internal */ hidden?: E
visible?: string
}`,
		},
		{
			name: "internal method with erased alias",
			source: `/** @stability experimental */
type E = string
export type Api = {
/** @internal */ hidden(): E
}`,
		},
		{
			name: "merged public property remains checked",
			source: prefix + `export interface Api {
/** @internal */ hidden?: E
}
export interface Api { hidden?: E }`,
			want: true,
		},
		{
			name: "merged internal properties",
			source: prefix + `export interface Api {
/** @internal */ hidden?: E
}
export interface Api {
/** @internal */ hidden?: E
}`,
		},
	}
	for _, test := range tests {
		for _, wholeProgram := range []bool{false, true} {
			mode := "file"
			if wholeProgram {
				mode = "program"
			}
			t.Run(test.name+"/"+mode, func(t *testing.T) {
				t.Parallel()
				files := map[string]string{"test.ts": test.source}
				messages := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
				if got := apiStabilityLifecycleContains(messages, "`Api` exposes"); got != test.want {
					t.Errorf("Api leak=%v, want %v: %v", got, test.want, messages)
				}
				control := apiStabilityLifecycleDiagnostics(t, files, false, wholeProgram)
				for _, message := range messages {
					if !strings.Contains(message, "effect(apiStabilityLeak)") && !apiStabilityLifecycleContains(control, message) {
						t.Errorf("rule introduced compiler diagnostic %q", message)
					}
				}
			})
		}
	}
}

// Mirrors Effect's HttpApiSchema augmentation of Schema.Annotations.Augment:
// the public status annotation remains visible, while the internal encoding
// and headers payloads must not lower the schema annotation surface's stability.
func TestApiStabilityLeakSchemaAnnotationAugmentation(t *testing.T) {
	t.Parallel()
	for _, publicEncoding := range []bool{false, true} {
		for _, wholeProgram := range []bool{false, true} {
			name := "internal"
			if publicEncoding {
				name = "public"
			}
			if wholeProgram {
				name += "/program"
			} else {
				name += "/file"
			}
			t.Run(name, func(t *testing.T) {
				t.Parallel()
				internalTag := "@internal"
				if publicEncoding {
					internalTag = "Encoding payload."
				}
				files := map[string]string{
					"schema.ts": `export namespace Annotations {
export interface Augment { readonly title?: string }
}
export interface Expanded extends Annotations.Augment {}
`,
					"test.ts": `import { Annotations } from "./schema"
/** @stability experimental */
interface Encoding { readonly kind: string }
/** @stability unstable */
interface WithHeadersAnnotation { readonly headers: object }
declare module "./schema" {
namespace Annotations {
interface Augment {
readonly httpApiStatus?: number | undefined
/** ` + internalTag + ` */
readonly "~httpApiEncoding"?: Encoding | undefined
/** @internal */
readonly "~httpApiWithHeaders"?: WithHeadersAnnotation | undefined
}
}
}
export interface Api extends Annotations.Augment {}
export declare const api: Api
export type Literal = {
/** @internal */ encoding?: Encoding
readonly httpApiStatus?: number
}
`,
				}
				messages := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
				for _, export := range []string{"Api", "api", "Expanded", "Annotations"} {
					if export == "Annotations" || export == "Expanded" {
						if !wholeProgram {
							continue
						}
					}
					if got := apiStabilityLifecycleContains(messages, "`"+export+"` exposes `Encoding`"); got != publicEncoding {
						t.Errorf("%s Encoding leak=%v, want %v: %v", export, got, publicEncoding, messages)
					}
				}
				for _, message := range messages {
					if strings.Contains(message, "exposes `WithHeadersAnnotation`") || strings.Contains(message, "`Literal` exposes") {
						t.Errorf("internal annotation payload leaked: %s", message)
					}
				}
			})
		}
	}
}
