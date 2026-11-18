#!/bin/bash
# Publishes Hersir's packages for one signed release, on Jeffrey's own computer:   scripts/packages.sh vX.Y.Z [aur]
# Run it right after scripts/sign-release.sh (docs/RELEASING.md, step 6). It publishes only what the release's signature
# covers:
# - It reads the release from GitHub and checks hersir-X.Y.Z.sig with ssh-keygen -Y verify against release-signers, over
#   the message sign-release.sh signs: the line "hersir vX.Y.Z <commit>" and a newline, where <commit> is the commit the
#   tag points to now, followed by the release's SHA256SUMS.
# - Every sha256 of a release's archive is its line in that signed SHA256SUMS. A package's other files come from that
#   commit, fetched by its id (from github.com), with their sha256s from git.
# - It fills the version, the sha256s and the commit into each package, shows what changes, asks Publish vX.Y.Z? [y/N]
#   once, and after y publishes each package with your own key.
# - No .sig, a signature that doesn't check (another key, a changed SHA256SUMS, the tag on another commit), or an answer
#   other than y: nothing is published, and one line on stderr says why.
# The packages, in this order (name some after the version to publish only those; GA-275 and GA-276 add Homebrew's and
# Scoop's, each as its own <name>_prepare, <name>_show and <name>_publish below):
# - aur: hersir-bin on the AUR, from packaging/aur/hersir-bin and packaging/LICENSE, pushed to
#   ssh://aur@aur.archlinux.org/hersir-bin.git with your AUR key (ssh takes it from ~/.ssh/config or ssh-agent).
# - homebrew: the formula hersir in the tap yotech-ai/tap, from packaging/homebrew and packaging/LICENSE, pushed to the
#   main branch of https://github.com/Yotech-AI/homebrew-tap.git with your own git login for GitHub.
# Only Jeffrey runs it, never CI or an agent: the AUR key never leaves Jeffrey's computer. For tests,
# HERSIR_RELEASE_SIGNERS replaces release-signers (a throwaway key's allowed-signers file), HERSIR_AUR_REPO the AUR's
# repository, HERSIR_TAP_REPO the tap's and HERSIR_SOURCE_REPO GitHub's, where the commit comes from (local
# repositories).
# A bash script for Linux and macOS, also for macOS's own bash 3.2. It needs gh, logged in to GitHub, git and ssh-keygen.

set -euo pipefail

# Every program this script runs (ssh-keygen, gh and git above all) comes from /usr/bin, on macOS also from
# /opt/homebrew/bin, when it is there, before the other folders on PATH: a program of the same name that something put
# in a folder earlier on PATH (~/.local/bin, say) never sees the release or the key. bash itself is /bin/bash, by the
# first line, for the same reason.
case "$OSTYPE" in
  darwin*) PATH="/usr/bin:/opt/homebrew/bin:$PATH" ;;
  *) PATH="/usr/bin:$PATH" ;;
esac
export PATH
# Everything from GitHub comes from github.com, and git works only in the repositories this script names.
unset GH_HOST CDPATH GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
  GIT_COMMON_DIR GIT_NAMESPACE GIT_CEILING_DIRECTORIES

REPO=Yotech-AI/hersir
NAMESPACE=hersir-release
PACKAGES="aur homebrew"
ROOT="$(cd -P "$(dirname "$0")/.." && pwd)"
NL=$'\n'
AUR_REPO="${HERSIR_AUR_REPO:-ssh://aur@aur.archlinux.org/hersir-bin.git}"
TAP_REPO="${HERSIR_TAP_REPO:-https://github.com/Yotech-AI/homebrew-tap.git}"
SOURCE_REPO="${HERSIR_SOURCE_REPO:-https://github.com/$REPO.git}"

say() { printf '%s\n' "$*"; }
# One line that says why, to stderr. Nothing is published after it.
fail() { printf '%s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'USAGE'
Publishes Hersir's packages for the signed release vX.Y.Z, right after scripts/sign-release.sh:
  scripts/packages.sh vX.Y.Z         checks the release's signature, fills its version, sha256s and commit into each
                                     package, shows the change, and publishes after y
  scripts/packages.sh vX.Y.Z aur     the same for the packages named: aur (hersir-bin on the AUR), homebrew (the
                                     formula hersir in Yotech-AI/homebrew-tap)
Nothing is published without a signature that checks against release-signers, or without y.
Environment, for tests with throwaway keys and local repositories: HERSIR_RELEASE_SIGNERS, an allowed-signers file
instead of release-signers; HERSIR_AUR_REPO, a git repository instead of the AUR's hersir-bin; HERSIR_TAP_REPO, a git
repository instead of Yotech-AI/homebrew-tap; HERSIR_SOURCE_REPO, a git repository instead of GitHub's, for the
release's commit.
It needs gh, logged in to GitHub (gh auth login), git and ssh-keygen, for the AUR your AUR key, and for the tap a git
login for GitHub that may push to Yotech-AI/homebrew-tap. docs/RELEASING.md, step 6, says when to run it.
USAGE
}

no_version="Give the release's version, like v1.2.3: scripts/packages.sh vX.Y.Z (see --help)."
[ "$#" -gt 0 ] || fail "$no_version"
tag="" chosen=""
for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
    -*) fail "$arg isn't an option of packages.sh (see --help)." ;;
    *)
      if [ -z "$tag" ]; then tag="$arg"; continue; fi
      case " $PACKAGES " in
        *" $arg "*) ;;
        *) fail "$arg isn't one of the packages ($PACKAGES), so nothing is published (see --help)." ;;
      esac
      case " $chosen " in *" $arg "*) ;; *) chosen="$chosen${chosen:+ }$arg" ;; esac ;;
  esac
done
[ -n "$tag" ] || fail "$no_version"
VERSION_RE='^v[0-9]+\.[0-9]+\.[0-9]+$'
[[ $tag =~ $VERSION_RE ]] || fail "$tag isn't a version like v1.2.3, so there is nothing to publish."
chosen="${chosen:-$PACKAGES}"

version="${tag#v}"
sig="hersir-$version.sig"

command -v ssh-keygen >/dev/null || fail "ssh-keygen isn't installed: it comes with OpenSSH."
command -v git >/dev/null || fail "git isn't installed."
command -v gh >/dev/null || fail "gh isn't installed: get it from https://cli.github.com, then run gh auth login."
if command -v sha256sum >/dev/null; then
  sha256() { sha256sum | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null; then
  sha256() { shasum -a 256 | cut -d ' ' -f 1; }
else
  fail "Neither sha256sum nor shasum is installed: sha256sum comes with coreutils."
fi
gh auth status --hostname github.com >/dev/null 2>&1 </dev/null ||
  fail "gh isn't logged in to GitHub: run gh auth login, then this again."

signers="${HERSIR_RELEASE_SIGNERS:-$ROOT/release-signers}"
[ -f "$signers" ] || fail "There is no $signers to check the signature against."

tmp="$(mktemp -d "${TMPDIR:-/tmp}/hersir-packages.XXXXXX")" || fail "Couldn't make a temp folder."
trap 'rm -rf "$tmp"' EXIT

# gh and git, which read nothing from the terminal (the answer is yours; ssh asks for your AUR key's passphrase on the
# terminal itself), with what they say on a failure kept for why.
github() { gh "$@" </dev/null 2>"$tmp/error"; }
quiet_git() { git "$@" </dev/null 2>"$tmp/error"; }
why() {
  local line
  line="$(grep -m 1 -v '^[[:space:]]*$' "$tmp/error" 2>/dev/null | LC_ALL=C tr -d '\000-\037\177' || true)"
  printf '%s' "${line:-it gave no reason}"
}

# ---------- the release, and its signature ----------

# The release's assets, and whether it is out: a draft's files aren't public yet.
info="$(github release view "$tag" --repo "$REPO" --json isDraft,isPrerelease,assets \
  --jq '(.isDraft | tostring), (.isPrerelease | tostring), .assets[].name')" ||
  fail "Couldn't read the release $tag of $REPO: $(why)"
draft="$(printf '%s\n' "$info" | sed -n 1p)"
prerelease="$(printf '%s\n' "$info" | sed -n 2p)"
assets="$(printf '%s\n' "$info" | sed 1,2d)"
listed() { case "$NL$assets$NL" in *"$NL$1$NL"*) return 0 ;; esac; return 1; }
[ "$draft" != true ] || fail "The release $tag is a draft, so nothing is published: its files aren't public yet."
[ "$prerelease" != true ] || fail "The release $tag is a pre-release, so nothing is published: packages follow releases."
listed "$sig" ||
  fail "The release $tag has no $sig: it isn't signed, so nothing is published. Sign it first: scripts/sign-release.sh $tag."
listed SHA256SUMS || fail "The release $tag has no SHA256SUMS, so nothing is published."

# The commit the tag points to now, peeled: a release's own tag points to it straight away, an annotated tag through a
# tag object (or a few). The signature must name this one.
ref="$(github api "repos/$REPO/git/ref/tags/$tag" --jq '.object.type + " " + .object.sha')" ||
  fail "Couldn't read the tag $tag of $REPO: $(why)"
peeled=0
while [ "${ref%% *}" = tag ] && [ "$peeled" -lt 5 ]; do
  peeled=$((peeled + 1))
  ref="$(github api "repos/$REPO/git/tags/${ref#* }" --jq '.object.type + " " + .object.sha')" ||
    fail "Couldn't read the tag $tag of $REPO: $(why)"
done
commit="${ref#* }"
SHA_RE='^[0-9a-f]{40}$'
if [ "${ref%% *}" != commit ] || ! [[ $commit =~ $SHA_RE ]]; then
  fail "The tag $tag doesn't lead to a commit, so nothing is published."
fi

mkdir "$tmp/release"
github release download "$tag" --repo "$REPO" --pattern SHA256SUMS --dir "$tmp/release" ||
  fail "Couldn't download the SHA256SUMS of $tag: $(why)"
github release download "$tag" --repo "$REPO" --pattern "$sig" --dir "$tmp/release" ||
  fail "Couldn't download $sig of $tag: $(why)"
sums="$tmp/release/SHA256SUMS"
[ -f "$sums" ] || fail "gh downloaded no SHA256SUMS of $tag."
[ -f "$tmp/release/$sig" ] || fail "gh downloaded no $sig of $tag."

# The message, byte for byte as sign-release.sh signs it, with the commit the tag points to now. A key that isn't in the
# signers file makes a signature that is sound over the message but not allowed: that one says so.
message="$tmp/hersir-$version"
{ printf 'hersir %s %s\n' "$tag" "$commit"; cat "$sums"; } > "$message"
if ! good="$(ssh-keygen -Y verify -f "$signers" -I "$NAMESPACE" -n "$NAMESPACE" -s "$tmp/release/$sig" <"$message" 2>/dev/null)"; then
  if ssh-keygen -Y check-novalidate -n "$NAMESPACE" -s "$tmp/release/$sig" <"$message" >/dev/null 2>&1; then
    fail "$sig is signed with a key that isn't in $signers, so nothing is published."
  fi
  fail "$sig doesn't sign this SHA256SUMS with the commit $tag points to ($commit): one of them changed since it was signed, so nothing is published."
fi
key="${good##* with }"

# The sha256 of the release's file $1: its one line in the signed SHA256SUMS, "<sha256>  <file name>".
signed_sum() {
  awk -v file="$1" '$2 == file && NF == 2 && $1 ~ /^[0-9a-f]+$/ && length($1) == 64 { print $1; n++ } END { exit n != 1 }' "$sums"
}

# The release's commit, from GitHub by its id, into a repository of this run's own: the files a package takes from it,
# and their sha256s, are git's, so they are that commit's.
source_git="$tmp/source.git"
quiet_git init --quiet --bare "$source_git" >/dev/null || fail "Couldn't make a git repository in $tmp: $(why)"
quiet_git -C "$source_git" fetch --quiet --depth=1 --no-tags "$SOURCE_REPO" "$commit" ||
  fail "Couldn't fetch the commit $commit from $SOURCE_REPO: $(why)"
[ "$(git -C "$source_git" cat-file -t "$commit" 2>/dev/null)" = commit ] ||
  fail "$SOURCE_REPO gave no commit $commit, so nothing is published."
# Its subject, without control characters: an escape sequence in it could make the terminal show something else.
subject="$(git -C "$source_git" log -1 --format=%s "$commit" 2>/dev/null | LC_ALL=C tr -d '\000-\037\177' || true)"
# The sha256 of the file $1 at the release's commit; fails when the commit has no such file.
commit_sum() {
  local sum
  sum="$(git -C "$source_git" cat-file blob "$commit:$1" 2>/dev/null | sha256)" || return 1
  printf '%s' "$sum"
}

# Whether the version $1 (X.Y.Z) comes after the version $2. Called only as a condition.
newer() {
  local a b c x y z
  IFS=. read -r a b c <<< "$1"
  IFS=. read -r x y z <<< "$2"
  if [ "$((10#$a))" -ne "$((10#$x))" ]; then [ "$((10#$a))" -gt "$((10#$x))" ]; return; fi
  if [ "$((10#$b))" -ne "$((10#$y))" ]; then [ "$((10#$b))" -gt "$((10#$y))" ]; return; fi
  [ "$((10#$c))" -gt "$((10#$z))" ]
}

# What git shows as the change in the repository $1, staged: no colours, no pager, and no control characters but tabs.
staged_change() {
  git -C "$1" -c color.ui=never --no-pager diff --cached --no-ext-diff 2>/dev/null | LC_ALL=C tr -d '\000-\010\013-\037\177'
}

# ---------- aur: hersir-bin on the AUR ----------
# packaging/aur/hersir-bin holds the PKGBUILD and its .SRCINFO for one release, and packaging/LICENSE (0BSD) goes
# beside them. Its values are the template's: for this release the version, pkgrel, the commit (also in the small
# files' addresses), the archive's sha256 from the signed SHA256SUMS, and each small file's sha256 from git at the
# commit, in the order of its sources.
AUR_TEMPLATE="$ROOT/packaging/aur/hersir-bin"
AUR_DIR="$tmp/aur"
aur_changed=0 aur_now="" aur_rel=1 aur_archive="hersir-$version-linux-x86_64.tar.gz" aur_archive_sum=""

# The template's PKGBUILD with this release's values.
aur_pkgbuild() {
  local line array="" files=0 archives=0 quoted_re="^(.*)'[0-9a-f]{64}'(.*)$"
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      pkgver=*) line="pkgver=$version" ;;
      pkgrel=*) line="pkgrel=$aur_rel" ;;
      _commit=*) line="_commit=$commit" ;;
      "sha256sums=("*) array=files ;;
      "sha256sums_x86_64=("*) array=archive ;;
    esac
    if [ -n "$array" ] && [[ $line =~ $quoted_re ]]; then
      if [ "$array" = files ]; then
        [ "$files" -lt "${#aur_paths[@]}" ] || return 1
        line="${BASH_REMATCH[1]}'${aur_sums[$files]}'${BASH_REMATCH[2]}"
        files=$((files + 1))
      else
        line="${BASH_REMATCH[1]}'$aur_archive_sum'${BASH_REMATCH[2]}"
        archives=$((archives + 1))
      fi
    fi
    case "$line" in *")") array="" ;; esac
    printf '%s\n' "$line"
  done < "$AUR_TEMPLATE/PKGBUILD"
  [ "$files" = "${#aur_paths[@]}" ] && [ "$archives" = 1 ]
}

# The template's .SRCINFO with this release's values, as makepkg --printsrcinfo writes it for the new PKGBUILD.
aur_srcinfo() {
  local line files=0 tab=$'\t'
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      "${tab}pkgver = "*) line="${tab}pkgver = $version" ;;
      "${tab}pkgrel = "*) line="${tab}pkgrel = $aur_rel" ;;
      "${tab}source = "* | "${tab}source_x86_64 = "*)
        line="${line//"$aur_old_version"/$version}"
        line="${line//"$aur_old_commit"/$commit}" ;;
      "${tab}sha256sums = "*)
        [ "$files" -lt "${#aur_paths[@]}" ] || return 1
        line="${tab}sha256sums = ${aur_sums[$files]}"
        files=$((files + 1)) ;;
      "${tab}sha256sums_x86_64 = "*) line="${tab}sha256sums_x86_64 = $aur_archive_sum" ;;
    esac
    printf '%s\n' "$line"
  done < "$AUR_TEMPLATE/.SRCINFO"
  [ "$files" = "${#aur_paths[@]}" ]
}

# The AUR repository's files for this release, with pkgrel $aur_rel, staged in its clone.
aur_write() {
  find "$AUR_DIR" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
  aur_pkgbuild > "$AUR_DIR/PKGBUILD" || fail "packaging/aur/hersir-bin/PKGBUILD's sums don't match its sources, so nothing is published."
  aur_srcinfo > "$AUR_DIR/.SRCINFO" || fail "packaging/aur/hersir-bin/.SRCINFO's sums don't match its sources, so nothing is published."
  cp "$ROOT/packaging/LICENSE" "$AUR_DIR/LICENSE" || fail "Couldn't copy packaging/LICENSE, so nothing is published."
  quiet_git -C "$AUR_DIR" add -A || fail "Couldn't stage hersir-bin's files: $(why)"
}

aur_prepare() {
  local tab=$'\t' line url raw path sum now_ver now_rel
  [ -f "$AUR_TEMPLATE/PKGBUILD" ] && [ -f "$AUR_TEMPLATE/.SRCINFO" ] ||
    fail "There is no $AUR_TEMPLATE/PKGBUILD and .SRCINFO to publish."
  listed "$aur_archive" || fail "The release $tag has no $aur_archive, so nothing is published."
  aur_archive_sum="$(signed_sum "$aur_archive")" ||
    fail "The signed SHA256SUMS of $tag has no one line for $aur_archive, so nothing is published."

  # The template's own release, and its small files: each a file of its commit, by the commit's id.
  aur_old_version="$(sed -n "s/^${tab}pkgver = //p" "$AUR_TEMPLATE/.SRCINFO" | head -n 1)"
  aur_old_commit="$(sed -n 's/^_commit=//p' "$AUR_TEMPLATE/PKGBUILD" | head -n 1)"
  [[ v$aur_old_version =~ $VERSION_RE ]] && [[ $aur_old_commit =~ $SHA_RE ]] ||
    fail "packaging/aur/hersir-bin has no version and commit of its own to replace, so nothing is published."
  raw="https://raw.githubusercontent.com/$REPO/$aur_old_commit/"
  aur_paths=() aur_sums=()
  while IFS= read -r line; do
    case "$line" in
      "${tab}source = "*)
        url="${line#*::}"
        case "$url" in
          "$raw"*) aur_paths+=("${url#"$raw"}") ;;
          *) fail "hersir-bin's source ${line#*= } isn't a file of the release's commit, so nothing is published." ;;
        esac ;;
    esac
  done < "$AUR_TEMPLATE/.SRCINFO"
  for path in ${aur_paths[@]+"${aur_paths[@]}"}; do
    sum="$(commit_sum "$path")" ||
      fail "The commit $commit has no $path, which hersir-bin installs, so nothing is published."
    aur_sums+=("$sum")
  done

  # The AUR's repository as it is: a package that isn't there yet clones empty, and its first push makes it.
  quiet_git -c init.defaultBranch=master clone --quiet "$AUR_REPO" "$AUR_DIR" >/dev/null ||
    fail "Couldn't clone $AUR_REPO, so nothing is published: $(why)"
  if [ -f "$AUR_DIR/.SRCINFO" ]; then
    now_ver="$(sed -n "s/^${tab}pkgver = //p" "$AUR_DIR/.SRCINFO" | head -n 1)"
    now_rel="$(sed -n "s/^${tab}pkgrel = //p" "$AUR_DIR/.SRCINFO" | head -n 1)"
    [[ v$now_ver =~ $VERSION_RE ]] && [[ $now_rel =~ ^[0-9]+$ ]] ||
      fail "hersir-bin on the AUR has the version '$now_ver-$now_rel', which isn't X.Y.Z-N: look at it first, nothing is published."
    if newer "$now_ver" "$version"; then
      fail "hersir-bin on the AUR is at $now_ver already, after $version, so nothing is published."
    fi
    aur_now="$now_ver-$now_rel"
    if [ "$now_ver" = "$version" ]; then aur_rel="$now_rel"; fi
  fi

  aur_write
  if ! git -C "$AUR_DIR" diff --cached --quiet 2>/dev/null; then
    aur_changed=1
    changed="$changed${changed:+ }aur"
    # The same release with other packaging files is a new pkgrel, so yay offers it.
    if [ "$aur_now" = "$version-$aur_rel" ]; then
      aur_rel=$((aur_rel + 1))
      aur_write
    fi
  fi

  # The PKGBUILD makes the launcher and the desktop entry from install.sh at the commit: it must manage that now, or
  # every build of the package would fail.
  (
    srcdir="$tmp/aur-check"
    mkdir -p "$srcdir"
    git -C "$source_git" cat-file blob "$commit:install.sh" > "$srcdir/hersir-$version-install.sh"
    # shellcheck disable=SC1091
    . "$AUR_DIR/PKGBUILD"
    _install_sh LAUNCH > "$srcdir/hersir-launch" && _install_sh DESKTOP > "$srcdir/hersir.desktop" &&
      [ -s "$srcdir/hersir-launch" ] && [ -s "$srcdir/hersir.desktop" ]
  ) >/dev/null 2>&1 ||
    fail "hersir-bin's PKGBUILD can't make the launcher and desktop entry from install.sh at $commit, so nothing is published."
}

aur_show() {
  say ""
  if [ -n "${HERSIR_AUR_REPO:-}" ]; then
    say "hersir-bin, in $AUR_REPO (HERSIR_AUR_REPO), not the AUR"
  else
    say "hersir-bin on the AUR ($AUR_REPO)"
  fi
  say "  Now         ${aur_now:-not on the AUR yet: this push makes it}"
  if [ "$aur_changed" = 0 ]; then
    say "  Stays       $aur_now: it has this release as it would be published, so it isn't published again"
    return 0
  fi
  say "  Becomes     $version-$aur_rel"
  say "  Archive     $aur_archive_sum  $aur_archive (the signed SHA256SUMS)"
  say "  Files       install.sh, the icons and NOTICE at $commit"
  say ""
  staged_change "$AUR_DIR"
}

aur_publish() {
  quiet_git -C "$AUR_DIR" commit --quiet -m "hersir-bin $version-$aur_rel" \
    -m "Hersir $tag at $commit: the release's Linux build, checked against its signed SHA256SUMS." >/dev/null ||
    fail "Couldn't commit hersir-bin $version-$aur_rel, so it isn't published: $(why)"
  quiet_git -C "$AUR_DIR" push --quiet origin HEAD:refs/heads/master >/dev/null ||
    fail "Couldn't push hersir-bin $version-$aur_rel to $AUR_REPO, so it isn't published: $(why)"
  say "Published hersir-bin $version-$aur_rel to $AUR_REPO: yay -Syu updates it (https://aur.archlinux.org/packages/hersir-bin)."
}

# ---------- homebrew: the formula hersir in the tap yotech-ai/tap ----------
# packaging/homebrew/hersir.rb is the formula for one release, and packaging/homebrew/README.md the tap's README. For this
# release the formula gets the address of the release's macOS archive and that archive's sha256 from the signed
# SHA256SUMS; Homebrew takes the version from the address. The tap gets it as Formula/hersir.rb, with the README and
# packaging/LICENSE (0BSD) beside it. Nothing else in the tap changes: it may hold other formulae.
TAP_TEMPLATE="$ROOT/packaging/homebrew"
TAP_DIR="$tmp/homebrew-tap"
tap_changed=0 tap_now="" tap_rev=0 tap_archive="hersir-$version-macos-arm64.tar.gz" tap_archive_sum=""

# The version Homebrew shows for the version $1 with the formula's revision $2: X.Y.Z, or X.Y.Z_N from revision 1 on.
homebrew_version() {
  if [ "$2" -gt 0 ]; then printf '%s_%s' "$1" "$2"; else printf '%s' "$1"; fi
}

# The template's formula with this release's values: its one url, the release's macOS archive, and its one sha256, that
# archive's line in the signed SHA256SUMS. A version line (the template has none: Homebrew reads the version from the
# url) gets the version. The revision is this script's: revision $tap_rev after the licence line, none for 0.
homebrew_formula() {
  local line urls=0 sums=0 licences=0 sum_re='^  sha256 "[0-9a-f]{64}"$'
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      '  url "'*) line="  url \"https://github.com/$REPO/releases/download/$tag/$tap_archive\""; urls=$((urls + 1)) ;;
      '  version "'*) line="  version \"$version\"" ;;
      '  revision '*) continue ;;
    esac
    if [[ $line =~ $sum_re ]]; then
      line="  sha256 \"$tap_archive_sum\""
      sums=$((sums + 1))
    fi
    printf '%s\n' "$line"
    case "$line" in
      '  license "'*)
        licences=$((licences + 1))
        if [ "$tap_rev" -gt 0 ]; then printf '  revision %s\n' "$tap_rev"; fi ;;
    esac
  done < "$TAP_TEMPLATE/hersir.rb"
  [ "$urls" = 1 ] && [ "$sums" = 1 ] && [ "$licences" = 1 ]
}

# The tap's files for this release, with the revision $tap_rev, staged in its clone: Formula/hersir.rb, README.md and
# LICENSE, each written anew (a link in their place goes, never what it points to).
homebrew_write() {
  local formulae="$TAP_DIR/Formula"
  if [ -L "$formulae" ] || { [ -e "$formulae" ] && [ ! -d "$formulae" ]; }; then
    fail "Formula in $TAP_REPO isn't a folder: look at the tap first, nothing is published."
  fi
  rm -rf "$formulae/hersir.rb" "$TAP_DIR/README.md" "$TAP_DIR/LICENSE"
  mkdir -p "$formulae" || fail "Couldn't make the tap's Formula folder in $TAP_DIR, so nothing is published."
  homebrew_formula > "$formulae/hersir.rb" ||
    fail "packaging/homebrew/hersir.rb has no one url, sha256 and license line to fill in, so nothing is published."
  cp "$TAP_TEMPLATE/README.md" "$TAP_DIR/README.md" && cp "$ROOT/packaging/LICENSE" "$TAP_DIR/LICENSE" ||
    fail "Couldn't copy packaging/homebrew/README.md and packaging/LICENSE, so nothing is published."
  quiet_git -C "$TAP_DIR" add -- Formula/hersir.rb README.md LICENSE || fail "Couldn't stage the tap's files: $(why)"
}

homebrew_prepare() {
  local formula="$TAP_DIR/Formula/hersir.rb" now_ver="" now_rev="" branch=""
  [ -f "$TAP_TEMPLATE/hersir.rb" ] && [ -f "$TAP_TEMPLATE/README.md" ] ||
    fail "There is no $TAP_TEMPLATE/hersir.rb and README.md to publish."
  listed "$tap_archive" || fail "The release $tag has no $tap_archive, so nothing is published."
  tap_archive_sum="$(signed_sum "$tap_archive")" ||
    fail "The signed SHA256SUMS of $tag has no one line for $tap_archive, so nothing is published."

  # The tap as it is, on main: a tap that is still empty clones empty, and its first push makes main.
  quiet_git -c init.defaultBranch=main clone --quiet "$TAP_REPO" "$TAP_DIR" >/dev/null ||
    fail "Couldn't clone $TAP_REPO, so nothing is published: $(why)"
  if git -C "$TAP_DIR" rev-parse --verify --quiet HEAD >/dev/null 2>&1; then
    branch="$(git -C "$TAP_DIR" symbolic-ref --short --quiet HEAD 2>/dev/null || true)"
    [ "$branch" = main ] ||
      fail "The tap $TAP_REPO is on the branch '$branch', not main: look at it first, nothing is published."
  fi
  # The tap's formula now: its version from its url, and its revision (none is 0). Only a file of the tap's own is read.
  if [ -e "$formula" ] || [ -L "$formula" ]; then
    if [ -f "$formula" ] && [ ! -L "$formula" ]; then
      now_ver="$(sed -n 's#^  url ".*/releases/download/v\([^/]*\)/.*$#\1#p' "$formula" 2>/dev/null | head -n 1 |
        LC_ALL=C tr -d '\000-\037\177')"
      now_rev="$(sed -n 's/^  revision \([0-9][0-9]*\)$/\1/p' "$formula" 2>/dev/null | head -n 1)"
    fi
    now_rev="$((10#${now_rev:-0}))"
    [[ v$now_ver =~ $VERSION_RE ]] ||
      fail "hersir in the tap has the version '$now_ver', which isn't X.Y.Z: look at it first, nothing is published."
    if newer "$now_ver" "$version"; then
      fail "hersir in the tap is at $now_ver already, after $version, so nothing is published."
    fi
    tap_now="$(homebrew_version "$now_ver" "$now_rev")"
    if [ "$now_ver" = "$version" ]; then tap_rev="$now_rev"; fi
  fi

  homebrew_write
  if ! git -C "$TAP_DIR" diff --cached --quiet 2>/dev/null; then
    tap_changed=1
    changed="$changed${changed:+ }homebrew"
    # The same release with another formula is the formula's next revision, so brew upgrade installs it again. A new
    # README or licence alone changes nothing brew installs.
    if [ "$tap_now" = "$(homebrew_version "$version" "$tap_rev")" ] &&
      ! git -C "$TAP_DIR" diff --cached --quiet -- Formula/hersir.rb 2>/dev/null; then
      tap_rev=$((tap_rev + 1))
      homebrew_write
    fi
  fi
}

homebrew_show() {
  say ""
  if [ -n "${HERSIR_TAP_REPO:-}" ]; then
    say "hersir, in $TAP_REPO (HERSIR_TAP_REPO), not the tap yotech-ai/tap"
  else
    say "hersir in the Homebrew tap yotech-ai/tap ($TAP_REPO)"
  fi
  say "  Now         ${tap_now:-not in the tap yet: this push puts it there}"
  if [ "$tap_changed" = 0 ]; then
    say "  Stays       $tap_now: it has this release as it would be published, so it isn't published again"
    return 0
  fi
  say "  Becomes     $(homebrew_version "$version" "$tap_rev")"
  say "  Archive     $tap_archive_sum  $tap_archive (the signed SHA256SUMS)"
  say ""
  staged_change "$TAP_DIR"
}

homebrew_publish() {
  local becomes
  becomes="$(homebrew_version "$version" "$tap_rev")"
  quiet_git -C "$TAP_DIR" commit --quiet -m "hersir $becomes" \
    -m "Hersir $tag at $commit: the release's macOS build, checked against its signed SHA256SUMS." >/dev/null ||
    fail "Couldn't commit hersir $becomes, so it isn't published: $(why)"
  quiet_git -C "$TAP_DIR" push --quiet origin HEAD:refs/heads/main >/dev/null ||
    fail "Couldn't push hersir $becomes to $TAP_REPO, so it isn't published: $(why)"
  say "Published hersir $becomes to $TAP_REPO: brew install yotech-ai/tap/hersir installs it, and brew update && brew upgrade hersir updates it."
}

# ---------- what changes, the answer, and publishing ----------

# Each package's <name>_prepare fills its files in a clone of its repository and adds its name to $changed when that
# clone changes; <name>_show says what changes; <name>_publish publishes it, after y.
changed=""
for name in $chosen; do "${name}_prepare"; done

say ""
say "Hersir's release $tag ($REPO), for its packages"
say "  Version     $tag"
say "  Commit      $commit ${subject:-(no subject)}"
say "  Signed      by $key, which $(basename "$signers") allows"
if [ -n "${HERSIR_RELEASE_SIGNERS:-}" ]; then
  say "  Checked by  $signers (HERSIR_RELEASE_SIGNERS), not release-signers"
fi
if [ -n "${HERSIR_SOURCE_REPO:-}" ]; then
  say "  Files from  $SOURCE_REPO (HERSIR_SOURCE_REPO), not GitHub"
fi
for name in $chosen; do "${name}_show"; done
say ""
if [ -z "$changed" ]; then
  say "Nothing to publish: each package has $tag already."
  exit 0
fi
printf 'Publish %s? [y/N] ' "$tag"
answer=""
read -r answer || true
[ "$answer" = y ] || fail "Nothing is published: the answer wasn't y."

for name in $changed; do "${name}_publish"; done
