#!/usr/bin/env bash
# Linker wrapper for building Rust code against Cosmopolitan Libc.
#
# Cargo passes many flags (e.g. -lunwind, dynamic-linking flags) that the
# cosmocc toolchain does not understand.  This script strips them out before
# forwarding to the real arch-specific compiler driver.
#
# Required environment variables:
#   COSMO  – path to the extracted cosmocc toolchain directory
#             (default: $HOME/cosmocc)
#   ARCH   – target architecture, "x86_64" or "aarch64"
#             (default: x86_64)

set -euo pipefail

COSMO="${COSMO:-$HOME/cosmocc}"
ARCH="${ARCH:-x86_64}"

declare -a args
args=()

for o in "$@"; do
    case "$o" in
        # Cosmopolitan always links statically; these flags are no-ops or
        # actively harmful when passed to cosmocc.
        -lunwind)          continue ;;
        -Wl,-Bdynamic)     continue ;;
        -Wl,-Bstatic)      continue ;;
        # Rust may request an eh-frame header; cosmocc handles its own.
        -Wl,--eh-frame-hdr) continue ;;
        # PIE / shared-object flags conflict with the static APE model.
        -pie)              continue ;;
        -Wl,-pie)          continue ;;
        # Relro / now are ELF-specific hardening flags unsupported by cosmocc.
        -Wl,-z,relro)      continue ;;
        -Wl,-z,now)        continue ;;
        -Wl,-z,noexecstack) continue ;;
        # Keep everything else.
        *)                 args+=("$o") ;;
    esac
done

exec "$COSMO/bin/$ARCH-unknown-cosmo-cc" "${args[@]}"
