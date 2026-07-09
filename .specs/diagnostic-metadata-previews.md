# Diagnostic Metadata Preview Generation

## Goal

Extend generated diagnostic metadata so each rule can include a preview payload derived from a real preview fixture and real diagnostic evaluation, matching the role of the upstream `metadata.json` preview data.

## Scope

- Covers generating the `preview` field for rules in `metadata.json`.
- Covers copying upstream diagnostic preview example files into the tsgo test corpus.
- Covers evaluating diagnostics in memory against those preview files during metadata generation tests.
- Covers the baseline/reference comparison for the enriched `metadata.json`.

## Relationship To Other Specs

This spec builds on [`.specs/diagnostic-metadata-generation.md`](../../.specs/diagnostic-metadata-generation.md).

That earlier spec covers grouped metadata structure, the `rules.json` to `metadata.json` rename, and README diagnostics table generation. This spec adds preview payload generation on top of that metadata foundation.

## Reference Implementation

The primary upstream references are:

- `.repos/effect-language-service/packages/language-service/scripts/codegen.ts`
- `.repos/effect-language-service/packages/language-service/test/metadata.test.ts`

The TypeScript test is the key reference for how preview fixtures are located, how diagnostics are evaluated in memory, and how the resulting spans and message text are embedded into generated metadata.

## Preview Source Files

For each rule that will expose a preview in tsgo metadata:

1. Start from the upstream `_preview.ts` diagnostic example file in the reference repository.
2. Copy that example into the tsgo test corpus under the appropriate Effect version.
3. Keep the version assignment aligned with the upstream source, using the v3 or v4 corpus according to where the preview exists upstream.

The preview fixture corpus is not handwritten from scratch when an upstream fixture already exists; the upstream fixture is the starting point for parity.

## Preview Payload Requirements

Each generated preview payload in `metadata.json` must include:

- `sourceText`: the preview source text used for evaluation
- `diagnostics`: the diagnostics produced for that preview, including start offset, end offset, and rendered diagnostic text

The preview data must come from actual tsgo diagnostic evaluation in memory, not from manually curated JSON.

If the preview source file contains leading test directives that should not appear in the published preview source text, the generated preview payload must trim those directives while keeping diagnostic offsets consistent with the trimmed content.

## Diagnostic Evaluation Flow

The metadata generation test must evaluate diagnostics for each preview file in memory.

The intended behavior is:

1. Load the preview fixture source.
2. Create the in-memory test environment needed to analyze that file in tsgo.
3. Run the relevant diagnostic rule against that preview file.
4. Collect the produced diagnostics in a stable order.
5. Convert them into the metadata preview payload shape.
6. Adjust offsets if leading directives are removed from the published preview `sourceText`.

This mirrors the role of the upstream JavaScript metadata test, but implemented in Go for this repository.

## Version Coverage

Preview fixtures may live in either the Effect v3 or Effect v4 test corpus depending on the upstream source fixture.

The generator/test flow must select the correct corpus for each rule instead of assuming all previews come from a single version.

## Testing and Reference Copy

The metadata generation test must continue to use the baseline/reference pattern.

For preview generation, that means:

1. Generate the full `metadata.json` structure in memory, including preview payloads.
2. Write the generated result to a local baseline file under `testdata/baselines/local/`.
3. Compare the local baseline against the committed package `metadata.json`.
4. Fail when they differ.

This test is the source of truth for preview-bearing metadata and must ensure preview drift is visible in review.

## Non-Goals

- Changing rule semantics solely to match upstream preview output.
- Introducing UI behavior that consumes previews; this spec is only about generating metadata.
- Replacing the separate README diagnostics table generation flow.
- Requiring preview support for rules that do not yet have a tsgo counterpart.

## Acceptance Criteria

1. Rules that participate in metadata previews expose a `preview` field in generated `metadata.json`.
2. Preview example files are copied from the upstream reference repository into the appropriate tsgo v3 or v4 test corpus before generation.
3. Preview payloads are produced by real in-memory tsgo diagnostic evaluation rather than handwritten data.
4. Preview diagnostics in metadata include stable start, end, and text fields.
5. The metadata baseline/reference test fails when preview payloads drift from generated output.
