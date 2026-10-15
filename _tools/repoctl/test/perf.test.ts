import assert from "node:assert/strict"
import test from "node:test"
import { median, numericMetric, parsePerfMetrics } from "../src/perf.ts"

test("parses aggregate performance diagnostics", () => {
  const metrics = parsePerfMetrics([
    "Projects built: 42",
    "Aggregate Parse time: 1.20s",
    "Aggregate Bind time: 0.30s",
    "Aggregate Check time: 4.50s",
    "Aggregate Emit time: 0.10s",
    "Aggregate Changes compute time: 0.20s",
    "Aggregate Total time: 6.30s",
    "Aggregate Memory used: 123456K",
    "Aggregate Memory allocs: 789",
    "Aggregate Symbols:   12849728",
    "Aggregate Types:   4050966",
    "Aggregate Instantiations:   14151659"
  ].join("\n"))

  assert.deepEqual(metrics, {
    projectsBuilt: "42",
    totalTime: "6.30s",
    checkTime: "4.50s",
    parseTime: "1.20s",
    bindTime: "0.30s",
    emitTime: "0.10s",
    changesTime: "0.20s",
    memoryUsed: "123456K",
    memoryAllocs: "789",
    symbols: "12849728",
    types: "4050966",
    instantiations: "14151659"
  })
})

test("calculates medians", () => {
  assert.equal(median([]), undefined)
  assert.equal(median([3, 1, 2]), 2)
  assert.equal(median([4, 1, 3, 2]), 2.5)
})

test("parses numeric metrics with and without a unit suffix", () => {
  assert.equal(numericMetric("1259683982", ""), 1259683982)
  assert.equal(numericMetric("22.368s", "s"), 22.368)
  assert.equal(numericMetric("s", "s"), undefined)
  assert.equal(numericMetric("", ""), undefined)
  assert.equal(numericMetric("12K", "s"), undefined)
  assert.equal(numericMetric(undefined, ""), undefined)
})
