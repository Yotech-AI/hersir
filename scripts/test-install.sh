#!/usr/bin/env bash
# Tests install.sh against a scratch HOME (never your real one): --check, an install of the programs already
# in target/release (which must back up existing data first), the installed command, then --uninstall (which must
# keep the data folder). A new install has the hersir names alone; an install from before 0.17.0 (lib/gizai) is updated
# where it is and keeps its gizai names as links to Hersir's (GA-248).
set -euo pipefail
cd "$(dirname "$0")/.."
T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
H="$T/home"; mkdir -p "$H"
run() { env -i HOME="$H" PATH="$PATH" USER="${USER:-hersir}" bash ./install.sh "$@"; }
fail() { echo "INSTALL TEST FAIL: $*"; exit 1; }
# the last line a program prints for --version: the launchers start Hersir through a login shell, which may print first
says() { "$@" --version 2>/dev/null | tail -n 1; }
want="hersir $(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
run --check > "$T/check.txt" 2>&1 || true
grep -q "Claude Code:" "$T/check.txt" || fail "--check printed no Claude Code line"
D="$H/.local/share/gizai"; mkdir -p "$D"
python3 -c "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute('create table t(x)'); c.execute('insert into t values (42)'); c.commit()" "$D/gizai.db"
run --skip-build > "$T/install.txt" 2>&1 || { cat "$T/install.txt"; fail "install failed"; }
ls "$D"/backups/gizai-before-install-*.db > /dev/null 2>&1 || { cat "$T/install.txt"; fail "no backup of the existing data before installing"; }
grep -q "Backed up your data" "$T/install.txt" || fail "the installer didn't say it backed up the data"
for f in .local/lib/hersir/hersir .local/lib/hersir/hersir-mcp .local/lib/hersir/hersir-launch .local/share/applications/hersir.desktop \
  .local/share/icons/hicolor/32x32/apps/hersir.png .local/share/icons/hicolor/128x128/apps/hersir.png .local/share/icons/hicolor/256x256/apps/hersir.png \
  .local/share/icons/hicolor/512x512/apps/hersir.png .local/share/icons/hicolor/scalable/apps/hersir.svg; do
  [ -e "$H/$f" ] || fail "missing $f"
done
[ -L "$H/.local/bin/hersir" ] || fail "no hersir command"
# a new install has the hersir names alone
for f in .local/lib/gizai .local/bin/gizai .local/share/applications/gizai.desktop; do
  [ ! -e "$H/$f" ] && [ ! -L "$H/$f" ] || fail "a new install has $f, a name from before 0.17.0"
done
[ -z "$(find "$H/.local/share/icons" -name 'gizai.*')" ] || fail "a new install has gizai icons"
grep -q "^Exec=$H/.local/lib/hersir/hersir-launch$" "$H/.local/share/applications/hersir.desktop" || fail "desktop entry Exec"
grep -q "^Icon=hersir$" "$H/.local/share/applications/hersir.desktop" || fail "desktop entry Icon"
grep -q "^StartupWMClass=hersir$" "$H/.local/share/applications/hersir.desktop" || fail "desktop entry StartupWMClass (Hyprland matches windows to it)"
if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$H/.local/share/applications/hersir.desktop" || fail "desktop entry is not valid"; fi
v="$(env -i HOME="$H" PATH="$PATH" "$H/.local/bin/hersir" --version | tail -n 1)"
[ "$v" = "$want" ] || fail "hersir --version said: $v"
grep -q "Done: hersir" "$T/install.txt" || fail "no Done line"
[ -z "$(find "$H/.local/lib/hersir" -name '.*.new.*')" ] || fail "the install left its temporary copies behind"
# --build-only (Hersir's own update builds with it, then installs with --skip-build); not together with --skip-build
run --help | grep -q -- "--build-only" || fail "--help doesn't list --build-only"
if run --build-only --skip-build > "$T/both.txt" 2>&1; then fail "--build-only --skip-build was accepted"; fi
grep -q "don't go together" "$T/both.txt" || { cat "$T/both.txt"; fail "no reason given for --build-only --skip-build"; }
# the install as Hersir's own update runs it: into another prefix, with the desktop entry and icons under it, after
# backing up the data there; nothing goes into HOME's ~/.local
U="$T/update-prefix"; mkdir -p "$U/share/gizai"
python3 -c "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute('create table t(x)'); c.commit()" "$U/share/gizai/gizai.db"
H2="$T/home2"; mkdir -p "$H2"
env -i HOME="$H2" PATH="$PATH" USER="${USER:-hersir}" HERSIR_PREFIX="$U" GIZAI_PREFIX="$U" XDG_DATA_HOME="$U/share" CARGO_TARGET_DIR="$PWD/target" \
  bash ./install.sh --skip-build > "$T/update.txt" 2>&1 || { cat "$T/update.txt"; fail "the update's install failed"; }
[ -x "$U/lib/hersir/hersir" ] && [ -x "$U/lib/hersir/hersir-mcp" ] && [ -x "$U/lib/hersir/hersir-launch" ] || fail "the update's install is incomplete"
grep -q "^Exec=$U/lib/hersir/hersir-launch$" "$U/share/applications/hersir.desktop" || fail "the update's desktop entry isn't under its prefix"
ls "$U"/share/gizai/backups/gizai-before-install-*.db > /dev/null 2>&1 || fail "the update's install didn't back up the data in its prefix"
[ ! -e "$H2/.local" ] || fail "the update's install wrote into HOME: $(find "$H2/.local" | head -5)"
# the prefix: HERSIR_PREFIX wins over GIZAI_PREFIX, which still works alone
P1="$T/prefix-hersir" P2="$T/prefix-gizai"
env -i HOME="$H2" PATH="$PATH" HERSIR_PREFIX="$P1" GIZAI_PREFIX="$P2" XDG_DATA_HOME="$T/share-both" bash ./install.sh --skip-build > "$T/both-prefixes.txt" 2>&1 \
  || { cat "$T/both-prefixes.txt"; fail "the install with both prefixes failed"; }
[ -x "$P1/lib/hersir/hersir" ] && [ ! -e "$P2" ] || fail "HERSIR_PREFIX didn't win over GIZAI_PREFIX"
env -i HOME="$H2" PATH="$PATH" GIZAI_PREFIX="$P2" XDG_DATA_HOME="$T/share-gizai" bash ./install.sh --skip-build > "$T/gizai-prefix.txt" 2>&1 \
  || { cat "$T/gizai-prefix.txt"; fail "the install with GIZAI_PREFIX alone failed"; }
[ -x "$P2/lib/hersir/hersir" ] || fail "GIZAI_PREFIX alone didn't set the prefix"
# data that can't be backed up stops the install before anything is replaced
before="$(stat -c %Y.%s "$H/.local/lib/hersir/hersir")"; sleep 1.1
echo "not a database" > "$D/gizai.db"
if run --skip-build > "$T/install2.txt" 2>&1; then fail "installed over data it could not back up"; fi
grep -q "nothing was installed" "$T/install2.txt" || { cat "$T/install2.txt"; fail "no reason given"; }
[ "$(stat -c %Y.%s "$H/.local/lib/hersir/hersir")" = "$before" ] || fail "the program was replaced after a failed backup"
echo keep > "$D/gizai.db"
run --uninstall > "$T/uninstall.txt" 2>&1 || fail "uninstall failed"
[ ! -e "$H/.local/lib/hersir" ] && [ ! -e "$H/.local/bin/hersir" ] && [ ! -e "$H/.local/share/applications/hersir.desktop" ] || fail "uninstall left files"
[ -z "$(find "$H/.local/share/icons" -name 'hersir.*' 2>/dev/null)" ] || fail "uninstall left icons"
[ -f "$H/.local/share/gizai/gizai.db" ] || fail "uninstall deleted the data"
run --uninstall --purge > /dev/null 2>&1
[ ! -e "$H/.local/share/gizai" ] || fail "--purge kept the data"

# An install from before 0.17.0, as 0.16.0's install.sh left it: lib/gizai with gizai, gizai-mcp and gizai-launch, the
# gizai command, gizai.desktop and the gizai icons, and the one line's source in gizai-src. It is updated where it is,
# as 0.16.0's own update runs install.sh: GIZAI_PREFIX and XDG_DATA_HOME=<prefix>/share, nothing else of Hersir's.
O="$T/old"; OL="$O/home/.local"; L="$OL/lib/gizai" S="$OL/share"
mkdir -p "$L" "$OL/bin" "$S/applications" "$S/icons/hicolor/scalable/apps" "$S/gizai/notes" "$S/gizai-src/target"
printf '#!/bin/sh\n[ "$1" = --version ] && echo "gizai 0.16.0"\nexit 0\n' > "$L/gizai"
printf '#!/bin/sh\nexit 0\n' > "$L/gizai-mcp"
printf '#!/bin/sh\nexec "%s/gizai" "$@"\n' "$L" > "$L/gizai-launch"
chmod 755 "$L/gizai" "$L/gizai-mcp" "$L/gizai-launch"
ln -s "$L/gizai-launch" "$OL/bin/gizai"
printf '[Desktop Entry]\nType=Application\nName=Hersir\nExec=%s/gizai-launch\nIcon=gizai\nStartupWMClass=gizai\n' "$L" > "$S/applications/gizai.desktop"
cp src-tauri/icons/gizai.svg "$S/icons/hicolor/scalable/apps/gizai.svg"
for size in 32 64 128 256 512; do mkdir -p "$S/icons/hicolor/${size}x${size}/apps"; cp src-tauri/icons/32x32.png "$S/icons/hicolor/${size}x${size}/apps/gizai.png"; done
echo "made before 0.17.0" > "$S/gizai/keep.txt"; echo "so was this" > "$S/gizai/notes/also.txt"
data_before="$(cd "$S/gizai" && find . -type f | LC_ALL=C sort | xargs cksum)"
[ "$(says "$OL/bin/gizai")" = "gizai 0.16.0" ] || fail "the 0.16.0-shaped install doesn't say gizai 0.16.0"
env -i HOME="$O/home" PATH="$PATH" GIZAI_PREFIX="$OL" XDG_DATA_HOME="$S" CARGO_TARGET_DIR="$PWD/target" TAURI_TELEMETRY_DISABLED=1 \
  bash ./install.sh --skip-build > "$T/old-update.txt" 2>&1 || { cat "$T/old-update.txt"; fail "the update of an install from before 0.17.0 failed"; }
# what 0.16.0's update checks (lib/gizai/gizai --version, its second word) and restarts (lib/gizai/gizai-launch), both
# commands, and Hersir's own names: all start the new Hersir
for p in "$L/gizai" "$L/gizai-launch" "$L/hersir" "$L/hersir-launch" "$OL/bin/gizai" "$OL/bin/hersir"; do
  [ "$(says "$p")" = "$want" ] || fail "$p says $(says "$p"), not $want"
done
[ -x "$L/hersir-mcp" ] && [ -x "$L/gizai-mcp" ] || fail "the chat's helper is missing under one of its names"
[ ! -e "$OL/lib/hersir" ] || fail "the update made lib/hersir: an install from before 0.17.0 stays in lib/gizai"
grep -q "^Exec=$L/hersir-launch$" "$S/applications/hersir.desktop" || fail "hersir.desktop doesn't start $L/hersir-launch"
[ "$(cd "$S/applications" && ls -- *.desktop)" = "hersir.desktop" ] || fail "the app launcher has more than one Hersir: $(ls "$S/applications")"
[ -z "$(find "$S/icons" -name 'gizai.*')" ] || fail "the update left gizai icons: $(find "$S/icons" -name 'gizai.*')"
[ -s "$S/icons/hicolor/scalable/apps/hersir.svg" ] || fail "no hersir.svg"
[ "$(cd "$S/gizai" && find . -type f | LC_ALL=C sort | xargs cksum)" = "$data_before" ] || fail "the update changed the data folder"
[ ! -e "$S/gizai-src" ] && [ ! -e "$S/hersir-src" ] || fail "the source copy from before 0.17.0, gizai-src, is still there"
grep -q "gizai command still works" "$T/old-update.txt" || fail "the update didn't say the gizai command still works"
# the next update (0.17.0's own, with both names of the prefix) finds the same install and keeps it working
env -i HOME="$O/home" PATH="$PATH" HERSIR_PREFIX="$OL" GIZAI_PREFIX="$OL" XDG_DATA_HOME="$S" bash ./install.sh --skip-build > "$T/old-update2.txt" 2>&1 \
  || { cat "$T/old-update2.txt"; fail "the next update of that install failed"; }
[ "$(says "$L/gizai-launch")" = "$want" ] && [ "$(says "$OL/bin/hersir")" = "$want" ] && [ ! -e "$OL/lib/hersir" ] || fail "the next update broke that install"
# --uninstall removes both layouts and keeps the data folder
env -i HOME="$O/home" PATH="$PATH" GIZAI_PREFIX="$OL" XDG_DATA_HOME="$S" bash ./install.sh --uninstall > "$T/old-uninstall.txt" 2>&1 || fail "uninstall of that install failed"
for p in "$L" "$OL/lib/hersir" "$OL/bin/gizai" "$OL/bin/hersir" "$S/applications/hersir.desktop" "$S/applications/gizai.desktop"; do
  [ ! -e "$p" ] && [ ! -L "$p" ] || fail "uninstall left $p"
done
[ -z "$(find "$S/icons" -name 'hersir.*' -o -name 'gizai.*')" ] || fail "uninstall left icons"
[ "$(cd "$S/gizai" && find . -type f | LC_ALL=C sort | xargs cksum)" = "$data_before" ] || fail "uninstall changed the data folder"
echo "INSTALL TEST OK ($v)"
