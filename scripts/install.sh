#!/usr/bin/env sh
# hostsctl installer.
#
#   curl -fsSL https://raw.githubusercontent.com/jtprogru/hostsctl/main/scripts/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version v0.1.0 --bin-dir ~/.local/bin
#
# POSIX sh, not bash: this has to run on an alpine container whose only shell
# is ash. The archive is verified against the release's checksums.txt before
# anything is unpacked.
set -eu

REPO="jtprogru/hostsctl"
VERSION=""
BIN_DIR="${HOSTSCTL_BIN_DIR:-/usr/local/bin}"
MUSL=""

usage() {
  cat <<'EOF'
Usage: install.sh [--version vX.Y.Z] [--bin-dir DIR] [--musl]

  --version   release to install (default: the latest one)
  --bin-dir   where to put the binary (default: /usr/local/bin)
  --musl      force the statically linked Linux build
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="${2:?--version needs a value}"
      shift 2
      ;;
    --bin-dir)
      BIN_DIR="${2:?--bin-dir needs a value}"
      shift 2
      ;;
    --musl)
      MUSL=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

die() {
  echo "install.sh: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required but not installed"
}

need tar
if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1" -o "$2"; }
  fetch_stdout() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -qO "$2" "$1"; }
  fetch_stdout() { wget -qO- "$1"; }
else
  die "either curl or wget is required"
fi

os="$(uname -s)"
arch="$(uname -m)"

case "$arch" in
  x86_64 | amd64) arch="x86_64" ;;
  aarch64 | arm64) arch="aarch64" ;;
  *) die "unsupported architecture: $arch" ;;
esac

case "$os" in
  Darwin) target="${arch}-apple-darwin" ;;
  Linux)
    libc="gnu"
    # A musl host has no glibc dynamic loader, so the gnu build would not run.
    if [ -z "$MUSL" ] &&
      ! [ -e /lib/ld-linux-x86-64.so.2 ] &&
      ! [ -e /lib/ld-linux-aarch64.so.1 ]; then
      MUSL=1
    fi
    if [ -n "$MUSL" ]; then
      libc="musl"
    fi
    target="${arch}-unknown-linux-${libc}"
    ;;
  *) die "unsupported operating system: $os (hostsctl manages /etc/hosts on Linux and macOS)" ;;
esac

if [ -z "$VERSION" ]; then
  VERSION="$(fetch_stdout "https://api.github.com/repos/${REPO}/releases/latest" |
    sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$VERSION" ] || die "cannot determine the latest release; pass --version"
fi

archive="hostsctl-${target}.tar.gz"
base="https://github.com/${REPO}/releases/download/${VERSION}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "hostsctl ${VERSION} — ${target}"
fetch "${base}/${archive}" "${tmp}/${archive}" || die "cannot download ${base}/${archive}"
fetch "${base}/checksums.txt" "${tmp}/checksums.txt" || die "cannot download checksums.txt"

# Verify before unpacking: an archive that does not match its published sum
# never gets extracted, let alone installed.
(
  cd "$tmp"
  want="$(grep " ${archive}\$" checksums.txt | awk '{print $1}')"
  [ -n "$want" ] || die "${archive} is not listed in checksums.txt"
  if command -v sha256sum >/dev/null 2>&1; then
    got="$(sha256sum "$archive" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    got="$(shasum -a 256 "$archive" | awk '{print $1}')"
  else
    die "neither sha256sum nor shasum is available to verify the download"
  fi
  [ "$want" = "$got" ] || die "checksum mismatch for ${archive}"
  echo "checksum ok"
)

tar -xzf "${tmp}/${archive}" -C "$tmp"
[ -f "${tmp}/hostsctl" ] || die "the archive does not contain a hostsctl binary"
chmod 0755 "${tmp}/hostsctl"

mkdir -p "$BIN_DIR" 2>/dev/null || true
if [ -w "$BIN_DIR" ]; then
  install -m 0755 "${tmp}/hostsctl" "${BIN_DIR}/hostsctl"
elif command -v sudo >/dev/null 2>&1; then
  echo "${BIN_DIR} is not writable, using sudo"
  sudo install -d "$BIN_DIR"
  sudo install -m 0755 "${tmp}/hostsctl" "${BIN_DIR}/hostsctl"
else
  die "${BIN_DIR} is not writable and sudo is not available; pass --bin-dir"
fi

echo "installed: ${BIN_DIR}/hostsctl"
"${BIN_DIR}/hostsctl" --version
cat <<'EOF'

Next:
  hostsctl init
  hostsctl add 127.0.0.1 my.local
  sudo hostsctl apply

Docs: https://jtprogru.github.io/hostsctl/
EOF
