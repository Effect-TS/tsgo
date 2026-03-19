#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_REPO="${1:-"$ROOT_DIR/.repos/effect-smol"}"
OUTPUT_ROOT="${OUTPUT_ROOT:-"$ROOT_DIR/_tools/.perf-out"}"
RUN_ID="${RUN_ID:-"$(date -u +%Y%m%dT%H%M%SZ)"}"
RUN_DIR="$OUTPUT_ROOT/$RUN_ID"
PATCHED_BIN="${PATCHED_BIN:-"$ROOT_DIR/tsgo"}"
STOCK_BIN="${STOCK_BIN:-"$TARGET_REPO/node_modules/.bin/tsgo"}"
CONFIG_PATH="${CONFIG_PATH:-tsconfig.json}"
RUNS="${RUNS:-1}"
DIAGNOSTICS_FLAG="${DIAGNOSTICS_FLAG:---diagnostics}"

mkdir -p "$RUN_DIR"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd pnpm

if [[ ! -d "$TARGET_REPO" ]]; then
  echo "Target repo does not exist: $TARGET_REPO" >&2
  exit 1
fi

if [[ ! -x "$PATCHED_BIN" ]]; then
  echo "Patched binary missing, building $PATCHED_BIN" >&2
  (
    cd "$ROOT_DIR"
    pnpm build:go
  )
fi

if [[ ! -x "$STOCK_BIN" ]]; then
  echo "Stock binary does not exist: $STOCK_BIN" >&2
  exit 1
fi

run_case() {
  local name="$1"
  local bin="$2"
  local run_number="$3"
  local case_dir="$RUN_DIR/$name/run-$run_number"
  local pprof_dir="$case_dir/pprof"
  local clean_log="$case_dir/clean.log"
  local stdout_log="$case_dir/stdout.log"
  local stderr_log="$case_dir/stderr.log"
  local diagnostics_log="$case_dir/diagnostics.log"
  local meta_log="$case_dir/meta.txt"

  mkdir -p "$pprof_dir"

  echo "[$name run $run_number] pnpm clean"
  (
    cd "$TARGET_REPO"
    pnpm clean
  ) >"$clean_log" 2>&1

  {
    echo "name=$name"
    echo "run=$run_number"
    echo "cwd=$TARGET_REPO"
    echo "binary=$bin"
    echo "config=$CONFIG_PATH"
    echo "pprofDir=$pprof_dir"
    echo "diagnosticsFlag=$DIAGNOSTICS_FLAG"
  } >"$meta_log"

  echo "[$name run $run_number] $bin -b $CONFIG_PATH $DIAGNOSTICS_FLAG --pprofDir $pprof_dir"
  if (
    cd "$TARGET_REPO"
    "$bin" -b "$CONFIG_PATH" "$DIAGNOSTICS_FLAG" --pprofDir "$pprof_dir"
  ) >"$stdout_log" 2>"$stderr_log"; then
    echo "exit=0" >>"$meta_log"
  else
    local status=$?
    echo "exit=$status" >>"$meta_log"
    echo "[$name run $run_number] failed with exit code $status" >&2
  fi

  cat "$stdout_log" "$stderr_log" >"$diagnostics_log"
}

print_summary() {
  local summary_file="$RUN_DIR/summary.txt"
  : >"$summary_file"

  for name in stock patched; do
    for run_number in $(seq 1 "$RUNS"); do
      local case_dir="$RUN_DIR/$name/run-$run_number"
      local diagnostics_log="$case_dir/diagnostics.log"
      local exit_code total_time check_time parse_time bind_time emit_time changes_time memory_used memory_allocs projects_built
      exit_code="$(sed -n 's/^exit=//p' "$case_dir/meta.txt" | tail -n 1)"
      total_time="$(grep -E '^Aggregate Total time:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Total time:[[:space:]]*//' || true)"
      check_time="$(grep -E '^Aggregate Check time:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Check time:[[:space:]]*//' || true)"
      parse_time="$(grep -E '^Aggregate Parse time:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Parse time:[[:space:]]*//' || true)"
      bind_time="$(grep -E '^Aggregate Bind time:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Bind time:[[:space:]]*//' || true)"
      emit_time="$(grep -E '^Aggregate Emit time:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Emit time:[[:space:]]*//' || true)"
      changes_time="$(grep -E '^Aggregate Changes compute time:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Changes compute time:[[:space:]]*//' || true)"
      memory_used="$(grep -E '^Aggregate Memory used:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Memory used:[[:space:]]*//' || true)"
      memory_allocs="$(grep -E '^Aggregate Memory allocs:' "$diagnostics_log" | tail -n 1 | sed 's/^Aggregate Memory allocs:[[:space:]]*//' || true)"
      projects_built="$(grep -E '^Projects built:' "$diagnostics_log" | tail -n 1 | sed 's/^Projects built:[[:space:]]*//' || true)"
      printf "%s run %s: exit=%s projects=%s total=%s check=%s parse=%s bind=%s emit=%s changes=%s memory=%s allocs=%s\n" \
        "$name" \
        "$run_number" \
        "${exit_code:-n/a}" \
        "${projects_built:-n/a}" \
        "${total_time:-n/a}" \
        "${check_time:-n/a}" \
        "${parse_time:-n/a}" \
        "${bind_time:-n/a}" \
        "${emit_time:-n/a}" \
        "${changes_time:-n/a}" \
        "${memory_used:-n/a}" \
        "${memory_allocs:-n/a}" | tee -a "$summary_file"
    done
  done
}

print_medians() {
  local summary_file="$RUN_DIR/summary.txt"

  for name in stock patched; do
    local rows_file="$RUN_DIR/$name-metrics.tsv"
    : >"$rows_file"

    for run_number in $(seq 1 "$RUNS"); do
      local case_dir="$RUN_DIR/$name/run-$run_number"
      local diagnostics_log="$case_dir/diagnostics.log"
      local total_time check_time memory_used
      total_time="$(grep -E '^Aggregate Total time:' "$diagnostics_log" | tail -n 1 | sed -E 's/^Aggregate Total time:[[:space:]]*([0-9.]+)s/\1/' || true)"
      check_time="$(grep -E '^Aggregate Check time:' "$diagnostics_log" | tail -n 1 | sed -E 's/^Aggregate Check time:[[:space:]]*([0-9.]+)s/\1/' || true)"
      memory_used="$(grep -E '^Aggregate Memory used:' "$diagnostics_log" | tail -n 1 | sed -E 's/^Aggregate Memory used:[[:space:]]*([0-9.]+)K/\1/' || true)"
      printf "%s\t%s\t%s\n" "${total_time:-nan}" "${check_time:-nan}" "${memory_used:-nan}" >>"$rows_file"
    done

    local total_median check_median memory_median
    total_median="$(cut -f1 "$rows_file" | grep -v '^nan$' | sort -n | awk '{ a[NR] = $1 } END { if (NR == 0) exit 1; if (NR % 2 == 1) printf "%.3f", a[(NR + 1) / 2]; else printf "%.3f", (a[NR / 2] + a[NR / 2 + 1]) / 2 }' || true)"
    check_median="$(cut -f2 "$rows_file" | grep -v '^nan$' | sort -n | awk '{ a[NR] = $1 } END { if (NR == 0) exit 1; if (NR % 2 == 1) printf "%.3f", a[(NR + 1) / 2]; else printf "%.3f", (a[NR / 2] + a[NR / 2 + 1]) / 2 }' || true)"
    memory_median="$(cut -f3 "$rows_file" | grep -v '^nan$' | sort -n | awk '{ a[NR] = $1 } END { if (NR == 0) exit 1; if (NR % 2 == 1) printf "%.0f", a[(NR + 1) / 2]; else printf "%.0f", (a[NR / 2] + a[NR / 2 + 1]) / 2 }' || true)"

    if [[ -z "$total_median" || -z "$check_median" || -z "$memory_median" ]]; then
      printf "%s median: unavailable\n" "$name" | tee -a "$summary_file"
      continue
    fi

    printf "%s median: total=%ss check=%ss memory=%sK\n" "$name" "$total_median" "$check_median" "$memory_median" | tee -a "$summary_file"
  done
}

for run_number in $(seq 1 "$RUNS"); do
  run_case stock "$STOCK_BIN" "$run_number"
  run_case patched "$PATCHED_BIN" "$run_number"
done

print_summary
print_medians

echo
echo "Artifacts written to: $RUN_DIR"
