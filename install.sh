#!/usr/bin/env bash
# Install OBSBOT Center on Ubuntu: system packages, Rust, a release build,
# an application menu entry, and a launch of the panel when a display exists.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE="${ROOT}/obsbot-core"
LAUNCHER_NAME="obsbot-center"
MINIMUM_RUST="1.81.0"
NEED_RELOGIN=0

APT_PACKAGES=(
  build-essential
  ca-certificates
  curl
  pkg-config
  libegl1-mesa-dev
  libgl1-mesa-dev
  libwayland-dev
  libxcb1-dev
  libxkbcommon-dev
)

log() {
  printf '%s\n' "$*"
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_user_linux() {
  [[ "$(uname -s)" == "Linux" ]] || die "This installer runs on Linux."
  [[ "$(id -u)" -ne 0 ]] || die "Run this as your normal user. It will ask for sudo when a package or group change needs it."
  command -v apt-get >/dev/null 2>&1 || die "This installer needs apt. Use Ubuntu or Debian."
  [[ -f "${CORE}/Cargo.toml" ]] || die "Missing ${CORE}/Cargo.toml. Run this script from a clone of the repository."
  [[ -r /etc/os-release ]] || die "Cannot read /etc/os-release."
  # shellcheck disable=SC1091
  . /etc/os-release
  case "${ID:-}-${ID_LIKE:-}" in
    ubuntu*|debian*|*-ubuntu*|*-debian*) ;;
    *) die "This installer supports Ubuntu and Debian. This system is ${ID:-unknown}." ;;
  esac
}

install_packages() {
  log "Installing compiler and window libraries..."
  sudo apt-get update
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y "${APT_PACKAGES[@]}"
}

rustc_version() {
  local compiler="$1"
  local line
  line="$("${compiler}" --version)"
  line="${line#rustc }"
  printf '%s\n' "${line%% *}"
}

rustc_is_new_enough() {
  local version="$1"
  local oldest
  oldest="$(printf '%s\n' "${MINIMUM_RUST}" "${version}" | sort -V | head -n 1)"
  [[ "${oldest}" == "${MINIMUM_RUST}" ]]
}

install_rust() {
  export PATH="${HOME}/.cargo/bin:${PATH}"
  local compiler=""
  if [[ -x "${HOME}/.cargo/bin/rustc" ]]; then
    compiler="${HOME}/.cargo/bin/rustc"
  elif command -v rustc >/dev/null 2>&1; then
    compiler="$(command -v rustc)"
  fi

  if [[ -n "${compiler}" ]] && rustc_is_new_enough "$(rustc_version "${compiler}")"; then
    log "Rust is already new enough: $(rustc_version "${compiler}")"
    return
  fi

  if command -v rustup >/dev/null 2>&1 || [[ -x "${HOME}/.cargo/bin/rustup" ]]; then
    log "Updating the stable Rust toolchain..."
    rustup toolchain install stable
    rustup default stable
    return
  fi

  log "Installing Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  export PATH="${HOME}/.cargo/bin:${PATH}"
}

ensure_video_group() {
  if id -nG "${USER}" | tr ' ' '\n' | grep -qx video; then
    return
  fi
  log "Adding ${USER} to the video group so the panel can open the camera."
  sudo usermod -aG video "${USER}"
  NEED_RELOGIN=1
}

build_panel() {
  log "Building the release panel. The first build downloads crates."
  (
    cd "${CORE}"
    cargo build --release --example panel --features panel
  )
}

install_launcher() {
  local binary="${CORE}/target/release/examples/panel"
  local applications="${HOME}/.local/share/applications"
  [[ -x "${binary}" ]] || die "The build did not produce ${binary}."
  mkdir -p "${HOME}/.local/bin" "${applications}"
  cat > "${HOME}/.local/bin/${LAUNCHER_NAME}" <<EOF
#!/bin/sh
exec '${binary}'
EOF
  chmod 755 "${HOME}/.local/bin/${LAUNCHER_NAME}"
  cat > "${applications}/${LAUNCHER_NAME}.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=OBSBOT Center
Comment=Tiny 2 Lite camera panel
Exec=${HOME}/.local/bin/${LAUNCHER_NAME}
Terminal=false
Categories=AudioVideo;Video;
EOF
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${applications}" >/dev/null 2>&1 || true
  fi
}

panel_is_running() {
  command -v pgrep >/dev/null 2>&1 && pgrep -x panel >/dev/null 2>&1
}

start_panel() {
  if [[ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
    log "This session has no display. Start the panel from the application menu, or run ${LAUNCHER_NAME}."
    return
  fi
  if panel_is_running; then
    log "The panel is already running."
    return
  fi
  log "Starting OBSBOT Center."
  nohup "${HOME}/.local/bin/${LAUNCHER_NAME}" >/tmp/obsbot-center.log 2>&1 &
  disown || true
}

main() {
  require_user_linux
  sudo -v
  install_packages
  install_rust
  ensure_video_group
  build_panel
  install_launcher
  if grep -qi microsoft /proc/version 2>/dev/null; then
    log "This is WSL. Attach the camera from Windows PowerShell first: usbipd attach --wsl --busid <BUSID>"
  elif command -v lsusb >/dev/null 2>&1 && ! lsusb -d 3564:fef9 >/dev/null 2>&1; then
    log "The Tiny 2 Lite is not on USB yet. Plug it in and the open window will pick it up."
  fi
  start_panel
  log "Command: ${HOME}/.local/bin/${LAUNCHER_NAME}"
  if [[ "${NEED_RELOGIN}" -eq 1 ]]; then
    log "Log out and back in if the preview stays on 'waiting for the camera'. The video group applies to a new login."
  fi
}

main "$@"
