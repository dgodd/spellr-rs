#!/usr/bin/env bash
# Linker wrapper for building Rust code against Cosmopolitan Libc.
#
# Cargo passes many flags (e.g. -lunwind, dynamic-linking flags) that the
# cosmocc toolchain does not understand.  This script strips them out before
# forwarding to the real arch-specific compiler driver.
#
# The cosmocc GCC driver should add cosmopolitan.a automatically via its spec
# files, but when invoked as a pure linker (only pre-compiled object files and
# rlib archives, no C source) it sometimes does not.  We therefore locate and
# append cosmopolitan.a explicitly so all C library symbols are always resolved.
#
# Required environment variables:
#   COSMO  – path to the extracted cosmocc toolchain directory
#             (default: $HOME/cosmocc)
#   ARCH   – target architecture, "x86_64" or "aarch64"
#             (default: x86_64)

COSMO="${COSMO:-$HOME/cosmocc}"
ARCH="${ARCH:-x86_64}"

DRIVER="$COSMO/bin/$ARCH-unknown-cosmo-cc"
if [ ! -x "$DRIVER" ]; then
    echo "cosmo-linker: fatal: cosmocc driver not found at $DRIVER" >&2
    echo "cosmo-linker: set the COSMO environment variable to your cosmocc directory" >&2
    exit 1
fi

declare -a args
args=()

for o in "$@"; do
    case "$o" in
        # Cosmopolitan always links statically; these flags are no-ops or
        # actively harmful when passed to cosmocc.
        -lunwind)           continue ;;
        -Wl,-Bdynamic)      continue ;;
        -Wl,-Bstatic)       continue ;;
        # Rust may request an eh-frame header; cosmocc handles its own.
        -Wl,--eh-frame-hdr) continue ;;
        # PIE / shared-object flags conflict with the static APE model.
        -pie)               continue ;;
        -Wl,-pie)           continue ;;
        # Relro / now are ELF-specific hardening flags unsupported by cosmocc.
        -Wl,-z,relro)       continue ;;
        -Wl,-z,now)         continue ;;
        -Wl,-z,noexecstack) continue ;;
        # Keep everything else.
        *)                  args+=("$o") ;;
    esac
done

# Explicitly append cosmopolitan.a so epoll_*, waitid, and all other C library
# symbols are available.  The find is kept separate from set -e so a missing
# directory doesn't silently kill the script.
COSMO_A=""
if [ -d "$COSMO" ]; then
    COSMO_A=$(find "$COSMO" -name "cosmopolitan.a" 2>/dev/null | head -1) || true
fi

if [ -n "$COSMO_A" ]; then
    args+=("$COSMO_A")
else
    echo "cosmo-linker: warning: could not locate cosmopolitan.a under $COSMO" >&2
fi

exec "$DRIVER" "${args[@]}"
