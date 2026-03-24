#!/usr/bin/env bash
# Licensed under the Apache-2.0 license
# SPDX-License-Identifier: Apache-2.0
#
# Run openprot library tests via Bazel. Prefer: bazelisk run //scripts:openprot_unit_test

set -euo pipefail

ROOT="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT"

BAZEL="${BAZEL:-bazelisk}"
if ! command -v "$BAZEL" &>/dev/null; then
  BAZEL="bazel"
fi

exec "$BAZEL" test //openprot:openprot_test "$@"
