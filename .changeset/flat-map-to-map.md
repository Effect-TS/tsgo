---
"@effect/tsgo": minor
---

Add the `flatMapToMap` diagnostic and quick fix, which replaces `Effect.flatMap` callbacks that only wrap their result with `Effect.succeed` with `Effect.map`. The diagnostic supports pipe, pipeable, data-first, and data-last forms.
