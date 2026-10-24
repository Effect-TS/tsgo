// @effect-v4
// @effect-diagnostics *:off

/** @stability experimental */
export interface PreviewExperimental {
  value: string
}

export interface PreviewStable {
  leaked: PreviewExperimental
}
