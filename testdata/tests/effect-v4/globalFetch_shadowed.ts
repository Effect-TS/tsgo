// @effect-diagnostics globalFetch:warning

const fetch = (input: string) => input

export const preview = fetch("https://example.com")
