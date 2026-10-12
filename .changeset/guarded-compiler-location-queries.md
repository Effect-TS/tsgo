---
"@effect/tsgo": minor
---

Route internal symbol and type location queries through TypeParser and enforce this with customlint. Reuse the existing TypeParser throughout pipeable signature comparison.

Prevent false TS2790 diagnostics for valid index-signature deletes such as `declare const map: { [key: string]: number }; delete map.x;`. Symbol queries skip deleted property accesses and their names, including parenthesized and optional targets. Receiver and computed-key queries remain available, and invalid required-property and readonly deletes still report compiler errors.
