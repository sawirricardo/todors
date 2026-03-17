#!/usr/bin/env bash
set -euo pipefail

REPO="${TODORS_REPO:-sawirricardo/todors}"
BINARY_NAME="todors"
INSTALL_DIR="${TODORS_INSTALL_DIR:-$HOME/.local/bin}"
REQUESTED_VERSION="${TODORS_VERSION:-latest}"

log() {
  printf '%s\n' "$*"
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

resolve_version() {
  if [[ "$REQUESTED_VERSION" != "latest" ]]; then
    printf '%s' "$REQUESTED_VERSION"
    return
  fi

  local tag
  tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  [[ -n "$tag" ]] || fail "could not resolve latest release tag for ${REPO}"
  printf '%s' "$tag"
}

resolve_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os="unknown-linux-musl" ;;
    Darwin) os="apple-darwin" ;;
    *)
      fail "unsupported OS: $os (supported: Linux, macOS)"
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch="x86_64" ;;
    arm64 | aarch64) arch="aarch64" ;;
    *)
      fail "unsupported architecture: $arch (supported: x86_64, aarch64)"
      ;;
  esac

  local target="${arch}-${os}"
  case "$target" in
    x86_64-unknown-linux-musl | x86_64-apple-darwin | aarch64-apple-darwin)
      printf '%s' "$target"
      ;;
    *)
      fail "no prebuilt binary for target: $target"
      ;;
  esac
}

verify_checksum() {
  local archive_path="$1"
  local checksum_file="$2"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$archive_path")" && sha256sum -c "$(basename "$checksum_file")")
  elif command -v shasum >/dev/null 2>&1; then
    local expected actual
    expected="$(awk '{print $1}' "$checksum_file")"
    actual="$(shasum -a 256 "$archive_path" | awk '{print $1}')"
    [[ "$expected" == "$actual" ]] || fail "checksum verification failed"
  else
    log "warning: skipping checksum verification (no sha256 tool found)"
  fi
}

main() {
  need_cmd curl
  need_cmd tar

  local version target asset archive_url checksum_url
  version="$(resolve_version)"
  target="$(resolve_target)"
  asset="${BINARY_NAME}-${target}.tar.gz"
  archive_url="https://github.com/${REPO}/releases/download/${version}/${asset}"
  checksum_url="${archive_url}.sha256"

  log "Installing ${BINARY_NAME} ${version} (${target}) from ${REPO}"

  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT

  local archive_path checksum_path
  archive_path="${tmp_dir}/${asset}"
  checksum_path="${archive_path}.sha256"

  curl -fL "$archive_url" -o "$archive_path"
  if curl -fsSL "$checksum_url" -o "$checksum_path"; then
    verify_checksum "$archive_path" "$checksum_path"
  else
    log "warning: checksum file not found; continuing without verification"
  fi

  tar -xzf "$archive_path" -C "$tmp_dir"
  [[ -f "${tmp_dir}/${BINARY_NAME}" ]] || fail "archive does not contain ${BINARY_NAME}"

  mkdir -p "$INSTALL_DIR"
  cp "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
  chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

  log "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
  if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    log "Add this to your shell profile if needed:"
    log "  export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
}

main "$@"
