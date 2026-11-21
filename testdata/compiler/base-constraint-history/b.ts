interface Box { a: Box; b: string }
function victim<T extends Box>(
  x: T,
  y: T["a"]["a"]["a"]["a"]["a"]["a"]["a"]["a"]["a"]["a"]
): string {
  return x.b;
}
