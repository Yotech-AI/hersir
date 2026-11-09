#!/usr/bin/env bash
# Hersir installer for Linux and macOS. Builds Hersir from source and installs it for you alone (no sudo):
#   ~/.local/lib/hersir/    hersir, hersir-mcp (the helper chat uses) and hersir-launch; on macOS also Hersir.app
#   ~/.local/bin/hersir     the command
#   Linux: a desktop entry and an icon, so Hersir shows up in your app launcher
#   macOS: a link to Hersir.app in ~/Applications, so Hersir starts from Finder and the Dock
# An install from before 0.17.0, in ~/.local/lib/gizai, is updated where it is: it gets the hersir names beside its
# gizai ones, which keep working (the gizai command too).
# Your data lives in ~/.local/share/hersir (macOS: ~/Library/Application Support/Hersir), or in the gizai folder of an
# install from before 0.17.0 (~/.local/share/gizai, ~/Library/Application Support/Gizai) when it holds gizai.db and
# there is no hersir folder: the folder Hersir uses. An install backs it up first and names it; only --uninstall --purge
# deletes it. On Windows, use install.ps1 (see the README).
#
# Usage:
#   ./install.sh                 from a checkout: check, build and install
#   curl -fsSL <raw url of this file> | bash
#                                clone (or update) the source into ~/.local/share/hersir-src, then the same
#   ./install.sh --check         only report what is missing
#   ./install.sh --build-only    check and build, install nothing (Hersir's own Update runs this, then --skip-build)
#   ./install.sh --skip-build    install the programs already built in target/release
#   ./install.sh --uninstall     remove Hersir, also an install from before 0.17.0 (add --purge to delete your data too)
# Environment: HERSIR_REPO (the git URL to clone), HERSIR_BRANCH (default production: the released code; main is
# development), HERSIR_PREFIX (default ~/.local), HERSIR_DATA_DIR, else GIZAI_DATA_DIR (macOS: another data folder).
# Their names from before 0.17.0, GIZAI_REPO, GIZAI_BRANCH and GIZAI_PREFIX, still work; a HERSIR_ one wins.
# It runs with macOS's own bash 3.2 and BSD tools too: no newer bash features or GNU-only options.
set -euo pipefail

DEFAULT_REPO="https://github.com/Yotech-AI/hersir.git"
# Each HERSIR_ variable wins over its name from before 0.17.0, GIZAI_, which still works alone.
REPO="${HERSIR_REPO:-${GIZAI_REPO:-$DEFAULT_REPO}}"
BRANCH="${HERSIR_BRANCH:-${GIZAI_BRANCH:-production}}"
PREFIX="${HERSIR_PREFIX:-${GIZAI_PREFIX:-$HOME/.local}}"
SHARE="${XDG_DATA_HOME:-$HOME/.local/share}"
# An install from before 0.17.0 (GA-248), with lib/gizai/gizai, is updated where it is, as 0.16.0's own update checks
# lib/gizai/gizai and restarts lib/gizai/gizai-launch: it gets the hersir names beside its gizai ones, which start the
# new Hersir from then on. A new install has the hersir names alone.
OLD_LIB="$PREFIX/lib/gizai"
if [ -e "$OLD_LIB/gizai" ] || [ -L "$OLD_LIB/gizai" ]; then OLD_LAYOUT=1 LIB="$OLD_LIB"; else OLD_LAYOUT=0 LIB="$PREFIX/lib/hersir"; fi
BIN="$PREFIX/bin"
APPS="$SHARE/applications"
ICONS="$SHARE/icons/hicolor"
# The data folder Hersir uses, which is what an install backs up and --purge deletes (GA-249): $1, the hersir folder,
# when it exists, else $2, the gizai one from before 0.17.0, when it holds gizai.db, else $1. Nothing is moved.
data_folder() { if [ ! -e "$1" ] && [ -f "$2/gizai.db" ]; then DATA="$2"; else DATA="$1"; fi; }
data_folder "$SHARE/hersir" "$SHARE/gizai"
SRC_CLONE="$SHARE/hersir-src"
# The source copy from before 0.17.0: an install removes it, unless it runs from it.
OLD_SRC_CLONE="$SHARE/gizai-src"
OS="$(uname -s)"
if [ "$OS" = Darwin ]; then
  # macOS keeps an app's data in ~/Library/Application Support, and apps in ~/Applications. The app was Gizai.app
  # before it was called Hersir (GA-233): an install or uninstall removes that one too, with the link to it it made.
  # HERSIR_DATA_DIR wins over GIZAI_DATA_DIR, its name before 0.17.0, which still works alone.
  data_folder "$HOME/Library/Application Support/Hersir" "$HOME/Library/Application Support/Gizai"
  DATA="${HERSIR_DATA_DIR:-${GIZAI_DATA_DIR:-$DATA}}"
  APP_LINK="$HOME/Applications/Hersir.app"
  OLD_APP_LINK="$HOME/Applications/Gizai.app"
  LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
fi

CHECK=0 BUILD_ONLY=0 SKIP_BUILD=0 UNINSTALL=0 PURGE=0
for arg in "$@"; do
  case "$arg" in
    --check) CHECK=1 ;;
    --build-only) BUILD_ONLY=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --purge) PURGE=1 ;;
    -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $arg (see --help)" >&2; exit 2 ;;
  esac
done
if [ "$BUILD_ONLY" = 1 ] && [ "$SKIP_BUILD" = 1 ]; then
  echo "--build-only and --skip-build don't go together (see --help)" >&2
  exit 2
fi

say() { printf '%s\n' "$*"; }
step() { printf '\n== %s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }
# Removes the link $1 when it is this install's: a link to $2.
remove_own_link() { if [ -L "$1" ] && [ "$(readlink "$1")" = "$2" ]; then rm -f "$1"; fi; }
# Removes the one line's source copy from before 0.17.0, gizai-src (it is hersir-src now), unless this runs from it.
remove_old_clone() {
  local old src
  old="$(cd "$OLD_SRC_CLONE" 2>/dev/null && pwd -P)" || return 0
  src="$(cd "$SRC" && pwd -P)" || return 0
  case "$src/" in "$old"/*) return 0 ;; esac
  rm -rf "$OLD_SRC_CLONE" || say "Note: could not remove $OLD_SRC_CLONE, the source copy from before 0.17.0. Remove it yourself."
}

case "$OS" in
  Linux|Darwin) ;;
  MINGW*|MSYS*|CYGWIN*) say "On Windows, install Hersir with install.ps1 in PowerShell (see the README)."; exit 1 ;;
  *) say "Hersir installs on Linux and macOS with this script, and on Windows with install.ps1."; exit 1 ;;
esac

# ---------- uninstall (macOS) ----------
if [ "$UNINSTALL" = 1 ] && [ "$OS" = Darwin ]; then
  step "Removing Hersir"
  # Both layouts: lib/hersir, and lib/gizai of an install from before 0.17.0, with their commands.
  rm -f "$BIN/hersir" "$BIN/gizai"
  # the links in ~/Applications only when they are this install's: to Hersir.app (in either layout), or to Gizai.app
  # (before GA-233)
  remove_own_link "$APP_LINK" "$PREFIX/lib/hersir/Hersir.app"
  remove_own_link "$APP_LINK" "$OLD_LIB/Hersir.app"
  remove_own_link "$OLD_APP_LINK" "$OLD_LIB/Gizai.app"
  rm -rf "$PREFIX/lib/hersir" "$OLD_LIB"
  say "Removed the app and the command."
  if [ "$PURGE" = 1 ]; then
    rm -rf "$DATA" "$SRC_CLONE" "$OLD_SRC_CLONE"
    say "Deleted your data ($DATA) and the source copy ($SRC_CLONE)."
  else
    say "Your data stays in $DATA (run with --uninstall --purge to delete it)."
  fi
  exit 0
fi

# ---------- uninstall ----------
if [ "$UNINSTALL" = 1 ]; then
  step "Removing Hersir"
  # Both layouts: Hersir's names, and the gizai ones of an install from before 0.17.0.
  for name in hersir gizai; do
    rm -f "$BIN/$name" "$APPS/$name.desktop" "$ICONS/scalable/apps/$name.svg"
    for size in 32 64 128 256 512; do rm -f "$ICONS/${size}x${size}/apps/$name.png"; done
  done
  rm -rf "$PREFIX/lib/hersir" "$OLD_LIB"
  have update-desktop-database && update-desktop-database "$APPS" >/dev/null 2>&1 || true
  # Refresh the icon cache only where one exists already (without one, GTK reads the folders directly).
[ -f "$ICONS/icon-theme.cache" ] && have gtk-update-icon-cache && gtk-update-icon-cache -q -t -f "$ICONS" >/dev/null 2>&1 || true
  say "Removed the app, the command, the desktop entry and the icon."
  if [ "$PURGE" = 1 ]; then
    rm -rf "$DATA" "$SRC_CLONE" "$OLD_SRC_CLONE"
    say "Deleted your data ($DATA) and the source copy ($SRC_CLONE)."
  else
    say "Your data stays in $DATA (run with --uninstall --purge to delete it)."
  fi
  exit 0
fi

# ---------- check ----------
step "Checking what Hersir needs"
missing_cmds=() missing_libs=() notes=()
node_from="your package manager, mise or nvm"
if [ "$OS" = Darwin ]; then
  # git and a C compiler come with Apple's Command Line Tools (until then, git and cc only offer to install them).
  xcode-select -p >/dev/null 2>&1 || notes+=("Apple's Command Line Tools (git and a C compiler) are missing: xcode-select --install")
  node_from="Homebrew (brew install node), mise, nvm or the installer from nodejs.org"
else
  for c in git pkg-config cc; do have "$c" || missing_cmds+=("$c"); done
fi
if [ "$SKIP_BUILD" = 0 ]; then
  have cargo || notes+=("Rust is missing: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   (then open a new terminal)")
  if have node; then
    major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
    [ "$major" -ge 20 ] || notes+=("Node.js 20 or newer is needed (found $(node -v)); install it with $node_from")
  else
    notes+=("Node.js 20 or newer is missing; install it with $node_from")
  fi
  have npm || notes+=("npm is missing (it comes with Node.js)")
fi
if [ "$OS" = Linux ]; then
  for lib in webkit2gtk-4.1 gtk+-3.0 librsvg-2.0 openssl; do
    have pkg-config && pkg-config --exists "$lib" 2>/dev/null || missing_libs+=("$lib")
  done
fi

distro=""
if [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  distro=" ${ID:-} ${ID_LIKE:-} "
fi
# The command that installs the missing system packages. CI's Install workflow runs it on Ubuntu, Fedora, Arch and
# openSUSE Tumbleweed (.github/workflows/install.yml). openSUSE's WebKitGTK 4.1 package was webkit2gtk3-devel and is
# webkitgtk3-devel on Tumbleweed: zypper finds it by its pkg-config name.
deps_command() {
  case "$distro" in
    *" arch "*|*" manjaro "*|*" endeavouros "*|*" omarchy "*) say "sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libayatana-appindicator librsvg git" ;;
    *" debian "*|*" ubuntu "*) say "sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config git" ;;
    *" fedora "*|*" rhel "*) say "sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel git && sudo dnf group install c-development" ;;
    *" opensuse"*|*" suse "*) say "sudo zypper in 'pkgconfig(webkit2gtk-4.1)' libopenssl-devel curl wget file libappindicator3-1 librsvg-devel git && sudo zypper in -t pattern devel_basis" ;;
    *) say "Install WebKitGTK 4.1, GTK 3, librsvg and OpenSSL development packages, a C compiler and pkg-config (see https://v2.tauri.app/start/prerequisites/)." ;;
  esac
}

ok=1
if [ "${#missing_cmds[@]}" -gt 0 ] || [ "${#missing_libs[@]}" -gt 0 ]; then
  ok=0
  # ${a[*]-} and ${a[@]+...}: bash before 4.4 (macOS has 3.2) calls an empty array unbound under set -u
  say "Missing system packages: ${missing_cmds[*]-} ${missing_libs[*]-}"
  say "Install them with:"
  say "  $(deps_command)"
fi
for n in ${notes[@]+"${notes[@]}"}; do ok=0; say "$n"; done
if have claude; then
  say "Claude Code: found ($(command -v claude)). Make sure you are logged in: run 'claude' once."
else
  say "Claude Code: not found. Hersir's agents and chat run it; install it from https://docs.claude.com/en/docs/claude-code and log in."
fi
# The tray icon loads libayatana-appindicator (or the older libappindicator) when Hersir starts; Hersir works without it.
# grep reads ldconfig's whole list: grep -q would stop at the first match, ldconfig would get SIGPIPE, and with pipefail
# the check would fail although the library is there.
if have ldconfig && ! ldconfig -p 2>/dev/null | grep -E 'lib(ayatana-)?appindicator3\.so\.1' >/dev/null; then
  say "Tray icon: libayatana-appindicator is missing, so Hersir will have no tray icon (Arch and Omarchy: sudo pacman -S libayatana-appindicator; Debian and Ubuntu: sudo apt install libayatana-appindicator3-1)."
fi
if [ "$OS" = Darwin ] && [ "$(uname -m)" != arm64 ]; then
  say "Note: Hersir is made for Macs with Apple silicon, and this shell runs as $(uname -m) (an Intel Mac, or Rosetta). The build may work, but isn't tested."
fi
[ "$ok" = 1 ] && say "Everything Hersir needs to build is here."
if [ "$CHECK" = 1 ]; then exit $((1 - ok)); fi
if [ "$ok" = 0 ] && [ "$SKIP_BUILD" = 0 ]; then
  say ""
  say "Install the missing pieces above, then run this again."
  exit 1
fi

# ---------- source ----------
here="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || true)"
if [ -n "$here" ] && [ -f "$here/src-tauri/tauri.conf.json" ] && grep -q '"productName": "Hersir"' "$here/src-tauri/tauri.conf.json"; then
  SRC="$here"
else
  step "Getting the source"
  if [ -z "$REPO" ]; then
    say "Set HERSIR_REPO to the git URL of Hersir, or run this script from a checkout."
    exit 1
  fi
  if [ -d "$SRC_CLONE/.git" ]; then
    git -C "$SRC_CLONE" pull --ff-only
  else
    git clone --depth 1 --branch "$BRANCH" "$REPO" "$SRC_CLONE"
  fi
  SRC="$SRC_CLONE"
fi
say "Source: $SRC"

# ---------- build ----------
if [ "$SKIP_BUILD" = 0 ]; then
  step "Building (this takes a few minutes the first time)"
  (
    cd "$SRC"
    export TAURI_TELEMETRY_DISABLED=1 npm_config_update_notifier=false npm_config_fund=false npm_config_audit=false
    npm ci
    if [ "$OS" = Darwin ]; then
      # macOS: Hersir.app too (target/release/bundle/macos), so Hersir starts from the Dock and Finder. Nothing is signed.
      npm run tauri build -- --bundles app
    else
      npm run tauri build -- --no-bundle
    fi
    # the crate gizai-mcp builds the program hersir-mcp
    cargo build --release -p gizai-mcp
  )
fi
for b in hersir hersir-mcp; do
  if [ ! -x "$SRC/target/release/$b" ]; then
    say "$SRC/target/release/$b is missing: build first (run without --skip-build)."
    exit 1
  fi
done
APP_BUILT="$SRC/target/release/bundle/macos/Hersir.app"
if [ "$OS" = Darwin ] && [ ! -x "$APP_BUILT/Contents/MacOS/hersir" ]; then
  say "$APP_BUILT is missing: build first (run without --skip-build)."
  exit 1
fi
if [ "$BUILD_ONLY" = 1 ]; then
  step "Built: $("$SRC/target/release/hersir" --version 2>/dev/null || echo hersir), in $SRC/target/release"
  say "Nothing was installed. Install it with: $SRC/install.sh --skip-build"
  exit 0
fi

# ---------- back up ----------
# Before replacing a Hersir you use, snapshot its data (the new build opens it, and may upgrade it, on its next start).
if [ -f "$DATA/gizai.db" ]; then
  step "Backing up your data"
  # The folder goes to the new build under both its names: a HERSIR_DATA_DIR of your own, which the build reads first and
  # this script reads only on macOS, can't send the backup elsewhere.
  if snap="$(HERSIR_DATA_DIR="$DATA" GIZAI_DATA_DIR="$DATA" "$SRC/target/release/hersir" --backup before-install 2>&1)"; then
    say "Backed up your data to $snap"
  else
    say "Could not back up $DATA/gizai.db: $snap"
    say "So nothing was installed. Move that file aside (or fix it), then run this again."
    exit 1
  fi
fi

# ---------- install (macOS) ----------
# Hersir.app holds both programs (Hersir looks for hersir-mcp next to itself). lib/hersir/hersir and hersir-mcp link into
# it, and hersir-launch, behind the hersir command, starts the app's own program, so macOS shows it as Hersir.
if [ "$OS" = Darwin ]; then
  step "Installing"
  mkdir -p "$LIB" "$BIN"
  # The new Hersir.app is put together under a temporary name of this installer's own first, then replaces the old one
  # with renames: a copy that fails (a full disk) leaves the installed Hersir as it was.
  new_app="$LIB/.Hersir.app.new.$$" old_app="$LIB/.Hersir.app.old.$$" new_launch="$LIB/.hersir-launch.new.$$"
  trap 'rm -rf "$new_app" "$new_launch"' EXIT
  if ! ditto "$APP_BUILT" "$new_app" || ! install -m 755 "$SRC/target/release/hersir-mcp" "$new_app/Contents/MacOS/hersir-mcp"; then
    say "Could not copy Hersir into $LIB, so nothing was installed."
    exit 1
  fi
  # Sign the app ad hoc again, so its signature covers hersir-mcp too (Apple silicon runs only signed programs, and a file
  # added after the build leaves a stale seal). An ad hoc signature names no one: Hersir stays unsigned in that sense.
  codesign --force --deep --sign - "$new_app" >/dev/null 2>&1 || true
  cat > "$new_launch" <<LAUNCH
#!/bin/sh
# Starts Hersir: the program inside Hersir.app, so macOS shows it as the app. Not a link to it: macOS tells a program the
# path of the link it was started from, and Hersir looks for hersir-mcp next to that. Hersir reads your login shell's
# PATH itself, so its agents find claude, git, npm and cargo also when it starts from the Dock.
exec "$LIB/Hersir.app/Contents/MacOS/hersir" "\$@"
LAUNCH
  chmod 755 "$new_launch"
  if [ -e "$LIB/Hersir.app" ]; then mv "$LIB/Hersir.app" "$old_app"; fi
  if ! mv "$new_app" "$LIB/Hersir.app"; then
    if [ -e "$old_app" ]; then mv "$old_app" "$LIB/Hersir.app"; fi
    say "Could not put the new Hersir.app into $LIB, so nothing was installed."
    exit 1
  fi
  rm -rf "$old_app"
  mv -f "$new_launch" "$LIB/hersir-launch"
  ln -sfn Hersir.app/Contents/MacOS/hersir "$LIB/hersir"
  ln -sfn Hersir.app/Contents/MacOS/hersir-mcp "$LIB/hersir-mcp"
  ln -sfn "$LIB/hersir-launch" "$BIN/hersir"
  # An install from before 0.17.0 keeps its gizai names, now links to Hersir's: the gizai command, and what 0.16.0's
  # own update checks (lib/gizai/gizai) and restarts (lib/gizai/gizai-launch), start the new Hersir.
  if [ "$OLD_LAYOUT" = 1 ]; then
    ln -sfn Hersir.app/Contents/MacOS/hersir "$LIB/gizai"
    ln -sfn Hersir.app/Contents/MacOS/hersir-mcp "$LIB/gizai-mcp"
    ln -sfn hersir-launch "$LIB/gizai-launch"
    ln -sfn "$LIB/hersir-launch" "$BIN/gizai"
  fi
  # LaunchServices learns the new app now, so its notifications come from Hersir (not Finder) from the first start.
  "$LSREGISTER" -f "$LIB/Hersir.app" >/dev/null 2>&1 || true
  # ~/Applications/Hersir.app for the usual install only: an install into another prefix (a test's) leaves yours alone.
  if [ "$PREFIX" = "$HOME/.local" ]; then
    if [ -L "$APP_LINK" ] || [ ! -e "$APP_LINK" ]; then
      mkdir -p "$HOME/Applications"
      ln -sfn "$LIB/Hersir.app" "$APP_LINK"
    else
      say "Note: $APP_LINK is there already and isn't a link, so it was left as it is. Hersir.app is in $LIB."
    fi
  fi
  # Gizai.app, the app before it was called Hersir (GA-233), goes now that nothing here links to it, and so does this
  # install's link to it in ~/Applications. LaunchServices forgets it first, as it has Hersir.app's id.
  remove_own_link "$OLD_APP_LINK" "$LIB/Gizai.app"
  if [ -e "$LIB/Gizai.app" ]; then
    "$LSREGISTER" -u "$LIB/Gizai.app" >/dev/null 2>&1 || true
    rm -rf "$LIB/Gizai.app" || say "Note: could not remove the old $LIB/Gizai.app (Hersir.app replaces it). Remove it yourself."
  fi
  remove_old_clone

  version="$("$LIB/hersir" --version 2>/dev/null || echo "hersir")"
  step "Done: $version"
  if [ -L "$APP_LINK" ] && [ "$(readlink "$APP_LINK")" = "$LIB/Hersir.app" ]; then
    say "Start Hersir from Applications in your home folder (Finder: Go > Home), and keep it in the Dock. Or run: hersir"
  else
    say "Start Hersir by opening $LIB/Hersir.app, or run: hersir"
  fi
  if [ "$OLD_LAYOUT" = 1 ]; then say "Your install from before 0.17.0 stays in $LIB, and the gizai command still works."; fi
  case ":$PATH:" in *":$BIN:"*) ;; *) say "Note: $BIN is not on your PATH; add it to use the hersir command." ;; esac
  say "Your data: $DATA"
  say "Update: Hersir offers new releases above Company in its sidebar (or run this installer again). Remove: run it with --uninstall."
  exit 0
fi

# ---------- install ----------
step "Installing"
mkdir -p "$LIB" "$BIN" "$APPS" "$ICONS/scalable/apps"
# Each program goes in under a temporary name of this installer's own first, then replaces the old one in one step (a
# rename): a copy that fails (a full disk) leaves the installed Hersir as it was.
new_hersir="$LIB/.hersir.new.$$" new_mcp="$LIB/.hersir-mcp.new.$$" new_launch="$LIB/.hersir-launch.new.$$"
trap 'rm -f "$new_hersir" "$new_mcp" "$new_launch"' EXIT
if ! install -m 755 "$SRC/target/release/hersir" "$new_hersir" || ! install -m 755 "$SRC/target/release/hersir-mcp" "$new_mcp"; then
  say "Could not copy Hersir into $LIB, so nothing was installed."
  exit 1
fi
cat > "$new_launch" <<LAUNCH
#!/bin/sh
# Starts Hersir with your login shell's PATH, so its agents find claude, git, npm and cargo even when
# Hersir is started from the app launcher. On machines without an NVIDIA GPU, WebKitGTK needs Mesa's EGL.
if ! lspci 2>/dev/null | grep -i nvidia >/dev/null && [ -f /usr/share/glvnd/egl_vendor.d/50_mesa.json ]; then
  export __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json
fi
if command -v bash >/dev/null 2>&1; then
  exec bash -lc 'exec "\$0" "\$@"' "$LIB/hersir" "\$@"
fi
exec "$LIB/hersir" "\$@"
LAUNCH
chmod 755 "$new_launch"
mv -f "$new_hersir" "$LIB/hersir"
mv -f "$new_mcp" "$LIB/hersir-mcp"
mv -f "$new_launch" "$LIB/hersir-launch"
ln -sfn "$LIB/hersir-launch" "$BIN/hersir"
# An install from before 0.17.0 keeps its gizai names, now links to Hersir's: the gizai command, and what 0.16.0's own
# update checks (lib/gizai/gizai) and restarts (lib/gizai/gizai-launch), start the new Hersir. The launcher starts it
# as hersir, so its windows match the desktop entry below.
if [ "$OLD_LAYOUT" = 1 ]; then
  ln -sfn hersir "$LIB/gizai"
  ln -sfn hersir-mcp "$LIB/gizai-mcp"
  ln -sfn hersir-launch "$LIB/gizai-launch"
  ln -sfn "$LIB/hersir-launch" "$BIN/gizai"
fi
# The app icon in every size launchers ask for, plus the SVG it is drawn from.
for pair in 32:32x32.png 64:64x64.png 128:128x128.png 256:128x128@2x.png 512:icon.png; do
  size="${pair%%:*}"; file="${pair#*:}"
  mkdir -p "$ICONS/${size}x${size}/apps"
  install -m 644 "$SRC/src-tauri/icons/$file" "$ICONS/${size}x${size}/apps/hersir.png"
done
install -m 644 "$SRC/src-tauri/icons/gizai.svg" "$ICONS/scalable/apps/hersir.svg"
# The window class is the program's name, hersir: StartupWMClass ties its windows to this entry.
cat > "$APPS/hersir.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Hersir
Comment=Clients, projects and tasks, worked on by local AI agents
Exec=$LIB/hersir-launch
Icon=hersir
Terminal=false
Categories=Development;ProjectManagement;
Keywords=tasks;kanban;agents;claude;projects;clients;
StartupWMClass=hersir
DESKTOP
# The desktop entry and icon from before 0.17.0 go, so the app launcher shows one Hersir.
rm -f "$APPS/gizai.desktop" "$ICONS/scalable/apps/gizai.svg"
for size in 32 64 128 256 512; do rm -f "$ICONS/${size}x${size}/apps/gizai.png"; done
have update-desktop-database && update-desktop-database "$APPS" >/dev/null 2>&1 || true
# Refresh the icon cache only where one exists already (without one, GTK reads the folders directly).
[ -f "$ICONS/icon-theme.cache" ] && have gtk-update-icon-cache && gtk-update-icon-cache -q -t -f "$ICONS" >/dev/null 2>&1 || true
remove_old_clone

version="$("$LIB/hersir" --version 2>/dev/null || echo "hersir")"
step "Done: $version"
say "Start Hersir from your app launcher, or run: hersir"
if [ "$OLD_LAYOUT" = 1 ]; then say "Your install from before 0.17.0 stays in $LIB, and the gizai command still works."; fi
case ":$PATH:" in *":$BIN:"*) ;; *) say "Note: $BIN is not on your PATH; add it to use the hersir command." ;; esac
say "Your data: $DATA"
say "Update: Hersir offers new releases above Company in its sidebar (or run this installer again). Remove: run it with --uninstall."
