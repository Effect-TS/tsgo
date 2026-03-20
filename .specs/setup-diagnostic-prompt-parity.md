# Setup Diagnostic Prompt Parity

## Goal

Adopt the upstream setup diagnostic prompt flow in the tsgo package once tsgo exposes the same generated diagnostic metadata structure expected by the reference implementation.

## Scope

- Covers the setup diagnostic prompt used when configuring diagnostic severities.
- Covers parity with the upstream prompt behavior and metadata contract.
- Assumes the grouped `metadata.json` structure and preview payloads already exist in tsgo.

## Relationship To Other Specs

This spec depends on:

- [`.specs/diagnostic-metadata-generation.md`](../../.specs/diagnostic-metadata-generation.md)
- [`.specs/diagnostic-metadata-previews.md`](../../.specs/diagnostic-metadata-previews.md)

Those specs establish the generated metadata structure that this prompt consumes.

## Reference Implementation

The canonical prompt implementation lives in the upstream reference repository:

- `.repos/effect-language-service/packages/language-service/src/cli/setup/diagnostic-prompt.ts`

The expectation for this work is to reuse that prompt behavior as directly as practical in tsgo once the metadata contract matches.

## Requirements

1. The tsgo setup flow must read diagnostic metadata from the generated `metadata.json` structure established by the metadata specs.
2. The setup diagnostic prompt should match the upstream prompt interaction model as closely as practical, including grouped browsing, search, preview rendering, and severity adjustment behavior.
3. The prompt adoption work should minimize local divergence from the upstream prompt logic so future parity updates stay straightforward.
4. The prompt must operate correctly against tsgo-generated metadata without introducing a separate tsgo-only metadata shape.

## Non-Goals

- Redefining the metadata format for setup.
- Reworking setup beyond the diagnostic prompt experience.
- Introducing a custom tsgo-only prompt design when upstream parity is sufficient.

## Acceptance Criteria

1. After metadata generation and preview generation are in place, tsgo can use the upstream-style diagnostic prompt against its own generated `metadata.json`.
2. The prompt behavior remains aligned with the upstream reference implementation rather than becoming a separate local design.
3. The prompt consumes the same metadata shape produced for README and metadata generation work.
