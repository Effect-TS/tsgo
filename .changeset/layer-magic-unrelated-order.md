---
"@effect/tsgo": patch
---

Fix layer magic ordering for unrelated layers so layers that only require services are composed after layers that provide no services.
