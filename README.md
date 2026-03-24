# OpenPRoT

## Technical Charter

The OpenPRoT Technical Charter can be found at
[<u>https://github.com/OpenPRoT/.github/blob/main/GOVERNANCE.md</u>](https://github.com/OpenPRoT/.github/blob/main/GOVERNANCE.md)

## Getting Started

NOTE: We are converting our build system to [bazel](https://bazel.build/).  We recommend installing [bazelisk](https://github.com/bazelbuild/bazelisk) to automatically manage bazel versions.

### Available Tasks


You can run tasks using the Pigweed workflow launcher `pw` or `bazel`.

- `./pw presubmit` - Run presubmit checks: formatting, license checks, C/C++ header checks and `clippy`.
- `./pw format` - Run the code formatters.
- `bazel test //...` - Run all tests.
- `bazel build //docs` - Build documentation.

### Coverage

Line coverage uses [**cargo-llvm-cov**](https://github.com/taiki-e/cargo-llvm-cov) via `//scripts:openprot_coverage`. Set up once: `rustup component add llvm-tools-preview`; optional HTML: install **lcov** so `genhtml` is available (e.g. `sudo apt install lcov` on Debian/Ubuntu).

- `bazelisk run //scripts:openprot_coverage` — writes `coverage.dat` (LCOV) at the repo root and, if `genhtml` exists, `coverage_html/index.html`.
- `bazelisk run //scripts:openprot_coverage -- -p openprot` — same, but only the `openprot` workspace package (extra args pass through to `cargo llvm-cov test`).
- `bazelisk coverage //openprot:openprot_test` — Bazel combined LCOV only; Rust line hits are often thin with the vendored toolchain, so prefer the command above for HTML reports.

Without Bazel, from the **repository root**: `cargo llvm-cov test --lcov --output-path coverage.dat` (install `cargo-llvm-cov` once if needed: `cargo install cargo-llvm-cov`). Optional: `genhtml --output-dir coverage_html coverage.dat`.


### Development

The project is structured as a bazel module.

## Requirements

- [Bazel](https://bazel.build/).  We recommend installing [bazelisk](https://github.com/bazelbuild/bazelisk) to automatically manage bazel versions.

No additional tools are required - all dependencies are managed by bazel.
