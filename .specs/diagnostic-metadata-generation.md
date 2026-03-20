# Diagnostic Metadata Generation

## Goal

Introduce a generated diagnostic metadata document for the tsgo implementation that matches the role and shape of the upstream `metadata.json`, so that diagnostic metadata is centralized, grouped, and consumed consistently by the setup flow and related tooling.

## Scope

- Covers generated diagnostic metadata for the tsgo language service package.
- Covers the metadata filename rename from `rules.json` to `metadata.json`.
- Covers metadata parity for existing rules that already exist in this repository.
- Covers generation of the root README diagnostics table from diagnostic metadata.
- Covers the baseline test pattern that generates a local reference copy and compares it against the committed package copy.
- Does not cover generating diagnostic preview payloads inside metadata; that is tracked separately.

## Reference Source of Truth

The canonical source for rule grouping values is the upstream `effect-language-service` repository in `.repos/effect-language-service`.

For every diagnostic rule that exists in both repositories, the tsgo metadata must use the same group value as the upstream reference repository. The mapping must be copied rule-by-rule rather than inferred loosely.

The upstream generation entrypoint for this metadata-and-README flow lives in:

- `.repos/effect-language-service/packages/language-service/scripts/codegen.ts`
- `.repos/effect-language-service/packages/language-service/test/metadata.test.ts`

Those files are the primary references for how grouped metadata and the generated README diagnostics table are derived from rule metadata in the reference implementation, including the expected rule ordering.

## Required Metadata Shape

The generated metadata document must be named `metadata.json`.

The generated document must include:

- A top-level list of diagnostic groups with stable identifiers and their human-readable metadata.
- A top-level list of rules.
- For each rule, metadata sufficient for setup and documentation consumers to reason about the rule without hardcoding separate tables.

For each rule that exists in tsgo, the metadata entry must include at least:

- Rule name
- Group
- Description
- Default severity

If the current tsgo metadata document already carries additional fields that remain useful to existing consumers, they may remain present as long as the generated file is renamed to `metadata.json` and the data model is updated to include group information in the same overall role as the upstream reference.

## Parity Requirements

1. Every existing tsgo rule must appear in the generated `metadata.json`.
2. Every rule that has an upstream counterpart must use the same group value as upstream.
3. The metadata filename used by consumers must be `metadata.json`, not `rules.json`.
4. Code that currently reads `rules.json` must instead read `metadata.json`.
5. The committed metadata file in the package source tree must reflect the generated output exactly.
6. Generated metadata rules must be ordered first by diagnostic group order and then by rule name within each group, matching the upstream reference behavior.

## README Diagnostics Table

The root `README.md` must contain markers that identify the generated diagnostics table section.

The diagnostics table inside that marked section must be generated from diagnostic metadata rather than maintained manually.

The generated table must match the same grouped shape used by the upstream `effect-language-service` implementation, including:

- Grouped diagnostic sections
- The same table structure and ordering model
- Rule rows derived from metadata rather than handwritten README content

This keeps documentation aligned with the generated metadata and avoids the rule list drifting from the actual source of truth.

## Testing and Reference Copy

The project must have a test that validates the generated metadata against a committed reference copy.

The test flow must follow this pattern for metadata:

1. Generate the metadata in memory.
2. Write the generated result to a local baseline file under `testdata/baselines/local/`.
3. Compare that local baseline file against the committed `metadata.json` that lives in the package source tree.
4. Fail when the generated baseline and the committed reference file differ.

The project must also have a test that validates the generated README diagnostics table against the committed repository README.

The README test flow must follow this pattern:

1. Read the committed root `README.md`, including the markers that bound the generated diagnostics table section.
2. Generate the diagnostics table from rule metadata and splice it into the marked section.
3. Write the generated README result to a local baseline file under `testdata/baselines/local/`.
4. Compare that local generated README against the committed root `README.md`.
5. Fail when the generated baseline and the committed README differ.

Both tests must follow the existing baseline/reference approach so that generated metadata and generated documentation remain intentional and reviewable.

## Non-Goals

- Changing diagnostic behavior, emitted diagnostics, or quick-fix behavior.
- Introducing new rules solely as part of this work.
- Redesigning setup UX beyond switching it to the renamed and enriched metadata source.
- Defining new grouping semantics that diverge from the upstream reference repository.
- Requiring manual editing of the generated diagnostics table contents inside the README.
- Generating or validating per-rule preview source text and preview diagnostic spans inside `metadata.json`.

## Acceptance Criteria

1. The generated diagnostics metadata file is named `metadata.json`.
2. Existing tsgo consumers load `metadata.json` instead of `rules.json`.
3. Every tsgo rule has a group value in generated metadata.
4. For rules shared with upstream, the group value matches the upstream reference repository exactly.
5. Generated metadata rules are ordered by group index first and by rule name second, matching the upstream reference behavior.
6. The metadata comparison test writes a local baseline and compares it with the committed package `metadata.json`.
7. The root README diagnostics table is generated from metadata within the README marker section and matches the grouped upstream shape.
8. The README comparison test writes a local generated README baseline and compares it with the committed root `README.md`.
9. The committed package `metadata.json` and committed root `README.md` can both be regenerated from project sources without manual editing of generated content.
