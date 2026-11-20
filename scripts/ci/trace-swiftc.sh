#!/bin/bash
# Run the Swift compiler under the CodeQL tracer.
#
# CodeQL traces macOS builds as x86_64 processes under Rosetta. Xcode and
# SwiftPM call this in place of swiftc, so Swift compilation alone pays that
# cost while the rest of the build stays native and untraced.
set -euo pipefail

exec "${CODEQL_RUNNER:?CodeQL tracing is not initialized}" "$(xcrun --find swiftc)" "$@"
