package rulerunner_test

import (
	"strings"
	"testing"
)

func TestApiStabilityLeakOptionalTaggedInheritance(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name    string
		members string
		extra   string
		want    bool
	}{
		{name: "empty base"},
		{name: "optional unstable property", members: "/** @stability unstable */ test?: string"},
		{name: "optional experimental property", members: "/** @stability experimental */ test?: string"},
		{name: "optional stable property", members: "/** @stability stable */ test?: string"},
		{name: "optional tagged method", members: "/** @stability unstable */ test?(): string"},
		{name: "optional internal property", members: "/** @internal */ test?: string"},
		{name: "required internal property", members: "/** @internal */ test: string"},
		{name: "public optional and internal required", members: "/** @stability unstable */ test?: string;\n/** @internal */ hidden: string"},
		{name: "required tagged property", members: "/** @stability unstable */ test: string", want: true},
		{name: "required tagged method", members: "/** @stability unstable */ test(): string", want: true},
		{name: "optional untagged property", members: "test?: string", want: true},
		{name: "mixed members", members: "/** @stability unstable */ test?: string; other?: string", want: true},
		{name: "invalid stability tag", members: "/** @stability unknown */ test?: string", want: true},
		{name: "index signature", members: "/** @stability unstable */ [key: string]: string", want: true},
		{name: "call signature", members: "/** @stability unstable */ (): string", want: true},
		{name: "construct signature", members: "/** @stability unstable */ new (): object", want: true},
		{name: "merged required member", members: "/** @stability unstable */ test?: string", extra: "interface Reportable { other: string }", want: true},
	}
	for _, test := range tests {
		for _, wholeProgram := range []bool{false, true} {
			mode := "file"
			if wholeProgram {
				mode = "program"
			}
			t.Run(test.name+"/"+mode, func(t *testing.T) {
				t.Parallel()
				source := "/** @stability unstable */\ninterface Reportable {\n" + test.members + "\n}\n" + test.extra + `
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}
export declare const expanded: Expanded
export function direct(e: Reportable) {}
export interface Ordinary extends Reportable {}
`
				files := map[string]string{"test.ts": source}
				messages := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
				control := apiStabilityLifecycleDiagnostics(t, files, false, wholeProgram)
				for _, name := range []string{"x", "Expanded", "expanded", "Ordinary"} {
					if got := apiStabilityLifecycleContains(messages, "`"+name+"` exposes `Reportable`"); got != test.want {
						t.Errorf("%s leak=%v, want %v: %v", name, got, test.want, messages)
					}
				}
				for _, name := range []string{"direct"} {
					if !apiStabilityLifecycleContains(messages, "`"+name+"` exposes `Reportable`") {
						t.Errorf("direct/ordinary exposure of %s must still report: %v", name, messages)
					}
				}
				for _, message := range messages {
					if !strings.Contains(message, "effect(apiStabilityLeak)") && !apiStabilityLifecycleContains(control, message) {
						t.Errorf("rule introduced compiler diagnostic %q", message)
					}
				}
				if !test.want {
					for _, name := range []string{"x", "Expanded", "expanded", "Ordinary"} {
						if apiStabilityLifecycleContains(messages, "`"+name+"` exposes") {
							t.Errorf("optional augmentation must not leak member tags for %s: %v", name, messages)
						}
					}
				}
			})
		}
	}
}

func TestApiStabilityLeakInheritancePaths(t *testing.T) {
	t.Parallel()
	prefix := `/** @stability unstable */
interface Reportable {
/** @stability unstable */ test?: string }
`
	tests := []struct {
		name   string
		source string
		dep    string
		want   bool
	}{
		{
			name: "module augmentation",
			dep:  "export interface Target {}",
			source: prefix + `import { Target } from "./dep"
declare module "./dep" { interface Target extends Reportable {} }
export function x(e: Target) {}
export interface Expanded extends Target {}`,
		},
		{
			name: "module augmentation of class",
			dep:  "export class Target {}",
			source: prefix + `import { Target } from "./dep"
declare module "./dep" { interface Target extends Reportable {} }
export function x(e: Target) {}
export interface Expanded extends Target {}`,
		},
		{
			name: "imported augmentation base",
			dep:  strings.Replace(prefix, "interface Reportable", "export interface Reportable", 1),
			source: `import { Reportable as Imported } from "./dep"
declare global { interface Error extends Imported {} }
export function x(e: Error) {}
export interface Expanded extends Error {}`,
		},
		{
			name: "ordinary and augmentation edges",
			dep:  strings.Replace(prefix, "interface Reportable", "export interface Reportable", 1) + "\nexport interface Target extends Reportable {}",
			source: `import { Target, Reportable } from "./dep"
declare module "./dep" { interface Target extends Reportable {} }
export function x(e: Target) {}
export interface Expanded extends Target {}`,
		},
		{
			name: "ordinary route beside ignored route",
			source: prefix + `declare global { interface Error extends Reportable {} }
interface Mixed extends Error, Reportable {}
export function x(e: Mixed) {}
export interface Expanded extends Mixed {}`,
		},
		{
			name: "inherited optional tagged members",
			source: `interface Parent {
/** @stability experimental */ parent?: string }
/** @stability unstable */
interface Reportable extends Parent {
/** @stability stable */ test?: string }
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}`,
		},
		{
			name: "inherited required member",
			source: `interface Parent { parent: string }
/** @stability unstable */
interface Reportable extends Parent {
/** @stability unstable */ test?: string }
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}`,
			want: true,
		},
		{
			name: "class base with optional tagged members",
			source: `/** @stability unstable */
class Reportable {
/** @stability unstable */ test?: string
}
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}`,
		},
		{
			name: "class inheriting augmented Error",
			source: prefix + `declare global { interface Error extends Reportable {} }
export class Expanded extends Error {}
export function x(e: Expanded) {}`,
		},
		{
			name: "generic augmentation arguments stay optional",
			source: `/** @stability experimental */
interface E { value: string }
/** @stability unstable */
interface Reportable<T> {
/** @stability unstable */ test?: T
}
declare global { interface Error extends Reportable<E> {} }
export function x(e: Error) {}
export interface Expanded extends Error {}`,
		},
		{
			name: "untagged base with tagged optional members",
			source: `interface Reportable {
/** @stability experimental */ test?: string
}
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}`,
		},
		{
			name: "own override remains exposed",
			source: prefix + `declare global { interface Error extends Reportable {} }
export interface Expanded extends Error {
/** @stability experimental */ test?: string }
export function x(e: Reportable) {}`,
			want: true,
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
				if test.dep != "" {
					files["dep.ts"] = test.dep
				}
				messages := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
				for _, name := range []string{"x", "Expanded"} {
					if got := apiStabilityLifecycleContains(messages, "`"+name+"` exposes"); got != test.want {
						t.Errorf("%s leak=%v, want %v: %v", name, got, test.want, messages)
					}
				}
			})
		}
	}
}
