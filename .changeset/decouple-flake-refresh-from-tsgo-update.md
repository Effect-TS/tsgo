---
"@effect/tsgo": patch
---

Update the automation so `refresh-flake-hash` runs as a reusable workflow after
`update-typescript-go` completes validation, instead of depending on PR events
triggered by the GitHub Actions bot.
