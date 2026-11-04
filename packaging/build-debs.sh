#!/usr/bin/env bash
# Build .deb packages for every Linux architecture Rust ships a musl std for.
#
# The binary is statically linked (musl + rust-lld), so a single cross-compile
# per architecture is enough: no cross libc, no cross C toolchain, no fakeroot
# dance. Debian architectures that Rust only ships *-linux-gnu std for
# (s390x, sparc64, mips*) are handled by .github/workflows/debs.yml with real
# cross toolchains.
#
# Usage:
#   ./packaging/build-debs.sh                       # all targets
#   ./packaging/build-debs.sh x86_64-unknown-linux-musl aarch64-unknown-linux-musl
#   OFFLINE=1 ./packaging/build-debs.sh            # never touch the crate mirror

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
DEB_VERSION="${VERSION}-1"
OUT="$ROOT/packaging/out"
mkdir -p "$OUT"

# Debian architecture -> Rust target.
declare -A ARCH_TARGET=(
  [amd64]="x86_64-unknown-linux-musl"
  [arm64]="aarch64-unknown-linux-musl"
  [armel]="arm-unknown-linux-musleabi"
  [armhf]="armv7-unknown-linux-musleabihf"
  [i386]="i686-unknown-linux-musl"
  [ppc64el]="powerpc64le-unknown-linux-musl"
  [riscv64]="riscv64gc-unknown-linux-musl"
  [loong64]="loongarch64-unknown-linux-musl"
  [ppc64]="powerpc64-unknown-linux-musl"
)

OFFLINE_FLAG=""
if [[ "${OFFLINE:-0}" == "1" ]]; then
  OFFLINE_FLAG="--offline"
fi

# Optional explicit target list, otherwise all known architectures.
declare -a WANTED=()
if [[ $# -gt 0 ]]; then
  WANTED=("$@")
else
  for arch in "${!ARCH_TARGET[@]}"; do WANTED+=("${ARCH_TARGET[$arch]}"); done
  # Stable, readable order.
  mapfile -t WANTED < <(printf '%s\n' "${WANTED[@]}" | sort)
fi

# Reverse lookup: target -> debian arch.
target_arch() {
  local t="$1"
  for a in "${!ARCH_TARGET[@]}"; do
    [[ "${ARCH_TARGET[$a]}" == "$t" ]] && { echo "$a"; return 0; }
  done
  echo "$t"
}

control_file() {
  local arch="$1" installed="$2"
  cat <<EOF
Package: bluetuith-rs
Version: ${DEB_VERSION}
Section: utils
Priority: optional
Architecture: ${arch}
Maintainer: bluetuith-rs contributors <bluetuith-rs@users.noreply.github.com>
Homepage: https://github.com/Novice-PG/bluetuith-rs
Installed-Size: ${installed}
Suggests: bluez, bluez-obexd
Description: full-featured Bluetooth manager for the terminal (ratatui)
 bluetuith-rs is a terminal UI for managing Bluetooth adapters and devices
 over BlueZ D-Bus: adapter control, pairing and connection, OBEX file
 transfers, AVRCP media playback, PAN networking and audio profiles.
 .
 Built with ratatui; supports mouse input, configurable keybindings, nine
 themes, search/filter and a --demo mode that needs no Bluetooth hardware.
 This package contains a statically linked binary with no runtime deps.
EOF
}

build_one() {
  local triple="$1" arch
  arch="$(target_arch "$triple")"
  echo ""
  echo "=================================================================="
  echo "  ${arch}  <-  ${triple}"
  echo "=================================================================="

  echo "-> rustup target add ${triple}"
  rustup target add "$triple"

  local envname
  envname="$(printf '%s' "$triple" | tr '[:lower:]-' '[:upper:]_')"
  echo "-> cargo build (linker=rust-lld)"
  env "CARGO_TARGET_${envname}_LINKER=rust-lld" \
    cargo build --release --target "$triple" --bin bluetuith-rs ${OFFLINE_FLAG:+$OFFLINE_FLAG}

  local bin="target/${triple}/release/bluetuith-rs"
  [[ -f "$bin" ]] || { echo "!! missing binary: $bin" >&2; return 1; }
  file "$bin"

  local pkg="$OUT/.stage-${arch}"
  rm -rf "$pkg"
  mkdir -p "$pkg/DEBIAN" \
           "$pkg/usr/bin" \
           "$pkg/usr/share/man/man1" \
           "$pkg/usr/share/doc/bluetuith-rs/examples" \
           "$pkg/usr/share/applications" \
           "$pkg/usr/share/bash-completion/completions" \
           "$pkg/usr/share/zsh/vendor-completions"

  install -m755 "$bin" "$pkg/usr/bin/bluetuith-rs"
  install -m644 debian/bluetuith-rs.1 "$pkg/usr/share/man/man1/bluetuith-rs.1"
  gzip -n -9 -f "$pkg/usr/share/man/man1/bluetuith-rs.1"
  install -m644 debian/changelog "$pkg/usr/share/doc/bluetuith-rs/changelog.Debian"
  gzip -n -9 -f "$pkg/usr/share/doc/bluetuith-rs/changelog.Debian"
  install -m644 debian/copyright "$pkg/usr/share/doc/bluetuith-rs/copyright"
  install -m644 README.md "$pkg/usr/share/doc/bluetuith-rs/README.md"
  install -m644 config.example.toml "$pkg/usr/share/doc/bluetuith-rs/examples/config.toml"
  install -m644 debian/bluetuith-rs.desktop "$pkg/usr/share/applications/bluetuith-rs.desktop"
  install -m644 debian/bluetuith-rs.bash-completion \
    "$pkg/usr/share/bash-completion/completions/bluetuith-rs"
  install -m644 debian/bluetuith-rs.zsh-completion \
    "$pkg/usr/share/zsh/vendor-completions/_bluetuith-rs"

  local installed
  installed="$(du -sk "$pkg/usr" | cut -f1)"
  control_file "$arch" "$installed" > "$pkg/DEBIAN/control"

  local deb="$OUT/bluetuith-rs_${VERSION}_${arch}.deb"
  echo "-> dpkg-deb --build"
  if ! dpkg-deb --build --root-owner-group "$pkg" "$deb" 2>/dev/null; then
    dpkg-deb --build "$pkg" "$deb"
  fi

  echo "-> verify"
  dpkg-deb --info "$deb" | sed -n '1,14p'
  dpkg-deb --contents "$deb" | grep -E 'usr/bin/bluetuith-rs|man1|desktop' || true
  rm -rf "$pkg"
  echo "OK: $deb ($(du -h "$deb" | cut -f1))"
}

declare -a BUILT=()
declare -a FAILED=()
for t in "${WANTED[@]}"; do
  if build_one "$t"; then
    BUILT+=("$t")
  else
    echo "!! FAILED: $t" >&2
    FAILED+=("$t")
  fi
done

echo ""
echo "=================================================================="
echo "  Summary: ${#BUILT[@]} built, ${#FAILED[@]} failed"
echo "=================================================================="
for t in "${BUILT[@]}"; do printf '  ok      %s\n' "$t"; done
for t in "${FAILED[@]}"; do printf '  FAILED  %s\n' "$t"; done
ls -1 "$OUT"/*.deb 2>/dev/null || true
( cd "$OUT" && sha256sum ./*.deb > SHA256SUMS 2>/dev/null || true )

[[ ${#FAILED[@]} -eq 0 ]]
