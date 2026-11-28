interface Box { a: Box; b: string }
function ten<U extends Box>(
  y: U["a"]["a"]["a"]["a"]["a"]["a"]["a"]["a"]["a"]["a"]
): string { return y.b; }
