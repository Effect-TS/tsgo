---
"@effect/tsgo": minor
---

Expose parser results through getters and readonly collection views. For Go consumers, `result.A` becomes `result.A()`, and `flow.Transformations[i]` becomes `flow.Transformations().At(i)`. Collection views support iteration and explicit owned copies. Compiler AST and type pointers remain borrowed.
