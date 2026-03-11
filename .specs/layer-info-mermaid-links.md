# Layer Info Mermaid Links

## Goal

Complete the layerInfo hover feature by adding clickable Mermaid diagram links to the Layer hover tooltip, and wiring the relevant plugin options so users can configure the behavior.

## Background

The layer graph infrastructure is already fully implemented: extraction, providers/requirers analysis, and all three Mermaid formatting functions (flat, nested, outline). The hover already displays a textual providers/requirers summary and Layer type parameters. What is missing is the generation of clickable Mermaid diagram URLs and the plugin options that control them.

## Scope

- Generating compressed Mermaid URLs for the full nested graph and the outline graph.
- Displaying those URLs as clickable links in the hover documentation.
- Exposing `mermaidProvider`, `noExternal`, and `layerGraphFollowDepth` as plugin options.
- Wiring `layerGraphFollowDepth` from plugin options into the hover path (currently hardcoded to 0).

## Requirements

### Mermaid URL Generation

1. The hover must include two clickable links after the providers/requirers summary: one labeled "Show full Layer graph" (using the nested graph format) and one labeled "Show Layer outline" (using the outline graph format).
2. Each link URL is built by:
   - JSON-serializing an object containing the Mermaid diagram code (e.g. `{"code":"flowchart TB\n..."}`)
   - Compressing the JSON bytes with zlib deflate (maximum compression)
   - Base64url-encoding the compressed bytes
   - Prepending `"pako:"` to the encoded string
   - Appending the result as the fragment of the Mermaid service URL (e.g. `https://mermaid.live/edit#pako:<encoded>`)
3. Links must be rendered using the `{@link <url> <label>}` syntax in the hover documentation so that IDEs display them as clickable.
4. When the `noExternal` option is `true`, both links must be omitted entirely.

### Plugin Options

5. A `mermaidProvider` option must be added to the plugin options, accepting the values `"mermaid.live"` (default), `"mermaid.com"`, or an arbitrary custom URL string.
   - `"mermaid.live"` resolves to `https://mermaid.live/edit#`
   - `"mermaid.com"` resolves to `https://www.mermaidchart.com/play#`
   - A custom string is used as-is with `/edit#` appended.
6. A `noExternal` boolean option must be added to the plugin options (default: `false`). When true, external links (Mermaid URLs) are suppressed in hover output.
7. A `layerGraphFollowDepth` integer option must be added to the plugin options (default: `0`). This controls how many levels deep the graph extraction follows symbol references when building the layer graph.
8. The hover path must read `layerGraphFollowDepth` from plugin options and pass it to the graph extraction call (instead of the current hardcoded zero).

### Hover Activation

9. The layer hover enrichment should only activate when the cursor is on the **name** of a variable or property declaration, not on arbitrary nodes within the initializer expression.

## Non-Goals

- Changes to the layer graph extraction, formatting, or providers/requirers analysis logic (already complete).
- Adding new Mermaid formatting styles beyond what already exists.
- The `quickinfoEffectParameters` and `quickinfoMaximumLength` options (separate concern, not specific to layerInfo).

## Acceptance Criteria

1. Hovering over a Layer-typed variable name shows the providers/requirers summary, Layer type parameters, and two clickable Mermaid links (full graph + outline).
2. The Mermaid links open the correct diagram when clicked in an IDE.
3. Setting `noExternal: true` suppresses both links without affecting the rest of the hover content.
4. Setting `mermaidProvider` to each supported value produces the correct base URL.
5. Setting `layerGraphFollowDepth` to a non-zero value causes the hover to follow symbol references to that depth.
6. Hovering on the initializer expression of a Layer variable (rather than the variable name) does not trigger the layer hover enrichment.
