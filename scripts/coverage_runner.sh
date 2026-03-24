#!/usr/bin/env bash
# Licensed under the Apache-2.0 license
# SPDX-License-Identifier: Apache-2.0
#
# LCOV + HTML coverage via cargo-llvm-cov. Prefer: bazelisk run //scripts:openprot_coverage
#
# Scope follows the root Cargo.toml workspace: `cargo llvm-cov test` from this repo root
# runs all [workspace] members by default. Narrow or adjust with standard cargo flags
# after bazel's "--", e.g. `-p openprot`, `--exclude mctp-lib`, `--workspace`.
#
# Prerequisites: rustup component llvm-tools-preview; optional: cargo install cargo-llvm-cov; genhtml (lcov package)

set -euo pipefail

ROOT="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
OUT="${ROOT}/coverage.dat"
HTML="${ROOT}/coverage_html"

if ! cargo llvm-cov --version &>/dev/null; then
  echo "Installing cargo-llvm-cov..."
  cargo install cargo-llvm-cov
fi

cd "$ROOT"
cargo llvm-cov test --lcov --output-path "$OUT" "$@"

if command -v genhtml &>/dev/null; then
  genhtml --output-dir "$HTML" "$OUT"
  echo "Coverage HTML: $HTML/index.html"
else
  echo "LCOV written to $OUT (install lcov for genhtml HTML reports)"
fi
