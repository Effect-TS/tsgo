type Variant =
  | { kind: "a"; value?: { id: string } }
  | { kind: "b"; value?: { id: string } }
  | { kind: "c"; value?: { id: string } }

declare const input: Variant

export const invalid = !input.value
