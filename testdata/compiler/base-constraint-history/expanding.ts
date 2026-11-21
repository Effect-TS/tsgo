type Expand<T extends { a: unknown }> =
  T extends unknown ? Expand<T["a"] & { a: unknown }> : never;
type Result = Expand<{ a: {} }>;
