---
"@effect/tsgo": minor
---

Add the `schemaClassObjectLiteral` correctness warning for object literals checked as Schema class instances in Effect v3 and v4. Spreading an instance, such as `const copy: User = { ...user }`, loses its prototype. Use `User.make({ ...user })` or `new User({ ...user })` to construct an instance.
