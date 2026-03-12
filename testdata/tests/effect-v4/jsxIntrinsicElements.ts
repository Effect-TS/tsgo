// @filename: tsconfig.json
{
  "compilerOptions": {
    "jsx": "react-jsx",
    "plugins": [
      {
        "name": "@effect/language-service",
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true
      }
    ]
  }
}

// @filename: /node_modules/react/jsx-runtime.d.ts
export namespace JSX {
  interface IntrinsicElements {
    div: { children?: any }
    span: { children?: any }
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {}
  }
}
export function jsx(type: any, props: any): JSX.Element
export function jsxs(type: any, props: any): JSX.Element

// @filename: /node_modules/react/package.json
{ "name": "react", "version": "19.0.0" }

// @filename: test.tsx
// Regression test: JSX intrinsic elements must not produce TS2304
// "Cannot find name" errors when the Effect plugin is enabled.
export const view = () => <div>Hello</div>

export const nested = () => (
  <div>
    <span>World</span>
  </div>
)
