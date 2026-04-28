#!/usr/bin/env bash
# scripts/enforce-coverage-floor.sh
# Phase 8 (08-03): per-file + project-wide branch threshold enforcer over lcov.info.
# cargo-llvm-cov 0.8.5 has --fail-under-lines but no --fail-under-branches and no
# per-file flag — this script fills both gaps by parsing the lcov record format.
# Usage: enforce-coverage-floor.sh <lcov-file> <project-branch-min> <file-line-min> <file-branch-min>
# Example: scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60
# Exit code: 0 on pass, 1 on any failure (CI surfaces as job failure).
set -euo pipefail
LCOV="${1:?lcov file path required}"
PROJ_BR_MIN="${2:?project branch min required}"
FILE_LN_MIN="${3:?per-file line min required}"
FILE_BR_MIN="${4:?per-file branch min required}"

awk -v project_br_min="$PROJ_BR_MIN" \
    -v file_ln_min="$FILE_LN_MIN" \
    -v file_br_min="$FILE_BR_MIN" '
  BEGIN { fail = 0; total_lf = 0; total_lh = 0; total_brf = 0; total_brh = 0 }
  /^SF:/    { sf  = substr($0, 4) }
  /^LF:/    { lf  = substr($0, 4) + 0 }
  /^LH:/    { lh  = substr($0, 4) + 0 }
  /^BRF:/   { brf = substr($0, 5) + 0 }
  /^BRH:/   { brh = substr($0, 5) + 0 }
  /^end_of_record/ {
    total_lf += lf; total_lh += lh
    total_brf += brf; total_brh += brh
    line_pct   = (lf  > 0) ? (100.0 * lh  / lf ) : 100.0
    branch_pct = (brf > 0) ? (100.0 * brh / brf) : 100.0
    if (line_pct < file_ln_min) {
      printf "FAIL: %s line coverage %.2f%% < floor %d%%\n", sf, line_pct, file_ln_min
      fail = 1
    }
    if (brf > 0 && branch_pct < file_br_min) {
      printf "FAIL: %s branch coverage %.2f%% < floor %d%%\n", sf, branch_pct, file_br_min
      fail = 1
    }
    sf=""; lf=0; lh=0; brf=0; brh=0
  }
  END {
    proj_br = (total_brf > 0) ? (100.0 * total_brh / total_brf) : 100.0
    if (proj_br < project_br_min) {
      printf "FAIL: project-wide branch coverage %.2f%% < gate %d%%\n", proj_br, project_br_min
      fail = 1
    }
    exit fail
  }
' "$LCOV"
