#!/bin/sh
# Hersir's one-line install for Linux and macOS:
#   curl -fsSL https://hersir.dev/install.sh | sh
# The website sends hersir.dev/install.sh on to this script, scripts/get.sh on the production branch, on GitHub.
# It gets the source of the latest release, the tag Hersir's own update fetches (docs/RELEASING.md), into
# ${XDG_DATA_HOME:-~/.local/share}/hersir-src, or updates it there. Before anything of the release runs, it checks the
# release's signature, hersir-X.Y.Z.sig, with ssh-keygen against Hersir's release keys, built in below: no signature,
# or one that doesn't check, and nothing is installed. Then it gets the release's ready-made build for this system,
# checks it against the release's SHA256SUMS and unpacks it into the source, and runs that release's install.sh with
# bash: install.sh --check --skip-build first, then install.sh --skip-build, which installs it. Without a ready-made
# build for this system, or when it can't be downloaded or doesn't match, install.sh builds the release's checked
# source instead, as it does from a clone. Run it again to update. Nothing goes into Hersir's data folder. The source
# folder is this script's own: changes in it are replaced. Before 0.17.0 it was gizai-src: that one becomes
# hersir-src, with its build, so the next build is quicker.
#
# Options go to install.sh, after sh -s --:   curl -fsSL <url> | sh -s -- --check   (see usage below)
# --from-source is the line's own: it builds the release's checked source, as before.
# Environment: HERSIR_PREFIX and XDG_DATA_HOME go to install.sh too (curl -fsSL <url> | HERSIR_PREFIX=$HOME/opt/hersir sh).
# HERSIR_REPO is the git repository, HERSIR_RELEASES_URL what it asks for the latest release (by default GitHub's; also
# http and file URLs), HERSIR_BUILDS_URL where it reads the release's signature, SHA256SUMS and ready-made builds (by
# default the release's own downloads on GitHub; also http and file URLs), and HERSIR_REF a branch or commit to get
# instead of the latest release, which is built from source without a signature (for tests and CI).
# HERSIR_RELEASE_SIGNERS, a file in OpenSSH's allowed-signers format, replaces the built-in keys: for tests, which sign
# fake releases with a throwaway key. Their names from before 0.17.0, GIZAI_PREFIX, GIZAI_REPO, GIZAI_RELEASES_URL,
# GIZAI_BUILDS_URL and GIZAI_REF, still work; a HERSIR_ one wins.
# A POSIX sh script, for dash and macOS's sh. It does everything in main, called on its last line, so a download cut
# short runs nothing.

# What this script gets comes from GitHub, so an install never depends on the website: the source from DEFAULT_REPO,
# the latest release from GitHub's API, its signature and builds from the release's downloads. The one line for
# Windows, which it only shows you, is hersir.dev's.
DEFAULT_REPO=https://github.com/Yotech-AI/hersir.git
GET_PS1=https://hersir.dev/install.ps1
NAMESPACE=hersir-release
NO_SSH_KEYGEN="Hersir checks each release's signature with ssh-keygen: install OpenSSH's client (openssh-client, openssh-clients or openssh), then run this again."

# Hersir's release keys: the lines of release-signers, at the repository's root, exactly as they are there (a test
# checks it). Each release's signature must check against one of them; keys never come from a release.
release_signers() {
  cat <<'SIGNERS'
hersir-release namespaces="hersir-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPyS5akxwuAio54EJUMpJgss6XvxkyznQ3sxZKpOm4mX Hersir release key
hersir-release namespaces="hersir-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJ9ER9k9DUooCPSOMQaWoha6E4IFW+e3t08ZmH1llz3J Hersir spare release key
SIGNERS
}

say() { printf '%s\n' "$*"; }
step() { printf '\n== %s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# "git", "git and bash", "git, bash and curl"
words() {
  case $# in
    1) printf '%s' "$1" ;;
    2) printf '%s and %s' "$1" "$2" ;;
    *) printf '%s, %s and %s' "$1" "$2" "$3" ;;
  esac
}

usage() {
  cat <<'USAGE'
Hersir's one-line install for Linux and macOS:
  curl -fsSL https://hersir.dev/install.sh | sh
It gets the source of Hersir's latest release into ~/.local/share/hersir-src (or updates it there) and checks the
release's signature against Hersir's release keys. Then it installs the release's ready-made build for your system,
with that release's install.sh: --check first, then the install. Without one, it builds the release's source. Run it
again to update.

Options go to install.sh, after sh -s --:
  curl -fsSL <url> | sh -s -- --check         check the release and show what is missing
  curl -fsSL <url> | sh -s -- --uninstall     remove Hersir; add --purge to delete your data and the source
  curl -fsSL <url> | sh -s -- --from-source   build the release's source instead of using its ready-made build
  --build-only and --skip-build work as they do for install.sh (see install.sh --help).
Environment: HERSIR_PREFIX (default ~/.local) and XDG_DATA_HOME, as for install.sh; HERSIR_REPO, HERSIR_RELEASES_URL,
HERSIR_BUILDS_URL (where the release's signature, SHA256SUMS and builds are), HERSIR_REF (a branch or commit to build
instead of the latest release, without a signature, for tests) and HERSIR_RELEASE_SIGNERS (an allowed-signers file
instead of Hersir's release keys, for tests). Their GIZAI_ names from before 0.17.0 still work.
USAGE
}

# git in the source folder's own repository only, never in one around it (a home folder kept in git, say).
src_git() { git -C "$dir" --git-dir="$dir/.git" --work-tree="$dir" "$@"; }

# Downloads the release's file $1 from $builds into $tmp, in at most $2 seconds. Returns 0 when it is there, 1 when the
# release has no such file (GitHub answers 404; a file:// one isn't there), and 2 when it couldn't be read: then
# $tmp/error says why. As for the latest release: -q first, so your ~/.curlrc plays no part, and redirects to https only.
get_file() {
  code="$(curl -q --silent --show-error --location --proto '=https,http,file' --proto-redir '=https' \
    --max-time "$2" --output "$tmp/$1" --write-out '%{http_code}' "$builds/$1" 2>"$tmp/error")"
  got=$?
  case "$got:$code" in
    0:200|0:000) return 0 ;;
    0:404|37:*) rm -f "$tmp/$1"; return 1 ;;
    0:*) say "the answer was HTTP $code" >"$tmp/error"; rm -f "$tmp/$1"; return 2 ;;
    *) rm -f "$tmp/$1"; return 2 ;;
  esac
}

# Why a download failed, from curl: its first line.
why() { sed -n '1{s/^curl: //;p;}' "$tmp/error" 2>/dev/null; }

# The sha256 of the file $1, in lowercase hex: with sha256sum (Linux), else shasum (macOS). Read from stdin, so the
# file's name plays no part in what they print.
sha256() {
  if have sha256sum; then sum="$(sha256sum <"$1")" || return 1
  elif have shasum; then sum="$(shasum -a 256 <"$1")" || return 1
  else return 1
  fi
  printf '%s\n' "${sum%% *}"
}

# The release's signature (docs/RELEASING.md, step 5): hersir-X.Y.Z.sig, made with one of Hersir's release keys over
# this message, byte for byte: the line "hersir vX.Y.Z <commit>" and a newline, where <commit> is the commit of the
# tag as fetched, followed by the release's SHA256SUMS as it is. When it doesn't check, one line says why, and nothing
# of the release runs.
check_signature() {
  step "Checking the signature of $tag"
  sig="hersir-$version.sig"
  for file in "$sig" SHA256SUMS; do
    get_file "$file" 60
    case $? in
      1) say "$tag isn't signed yet: try again in a few minutes."; return 1 ;;
      2) say "Could not download $file of $tag from $builds ($(why)). Check your internet connection, then run this again."; return 1 ;;
    esac
  done
  signers="${HERSIR_RELEASE_SIGNERS:-}"
  if [ -n "$signers" ]; then
    if [ ! -f "$signers" ]; then
      say "HERSIR_RELEASE_SIGNERS is $signers, which isn't a file, so the signature of $tag can't be checked."
      return 1
    fi
    say "Checked against $signers (HERSIR_RELEASE_SIGNERS), not Hersir's release keys."
  else
    signers="$tmp/release-signers"
    release_signers >"$signers"
  fi
  { printf 'hersir %s %s\n' "$tag" "$commit"; cat "$tmp/SHA256SUMS"; } >"$tmp/message"
  if ssh-keygen -Y verify -f "$signers" -I "$NAMESPACE" -n "$NAMESPACE" -s "$tmp/$sig" <"$tmp/message" >"$tmp/verify" 2>&1; then
    say "$tag is signed with Hersir's release key, for its commit $commit."
    return 0
  fi
  # An ssh-keygen from before OpenSSH 8.1 has no -Y.
  case "$(cat "$tmp/verify")" in
    *"option -- Y"*|*"option -- 'Y'"*) say "$NO_SSH_KEYGEN"; return 1 ;;
  esac
  # A signature by one of the keys that doesn't check was made over another message: another commit, or another
  # SHA256SUMS.
  if ssh-keygen -Y find-principals -f "$signers" -s "$tmp/$sig" >/dev/null 2>&1; then
    say "The signature of $tag is by Hersir's release key, but not for its commit $commit and its SHA256SUMS: this isn't the release that was signed, so nothing was installed."
  else
    say "The signature of $tag doesn't check against Hersir's release keys, so nothing was installed."
  fi
  return 1
}

# Puts the release's ready-made build for this system into the source folder: the archive its SHA256SUMS lists,
# downloaded, checked against SHA256SUMS and unpacked where the build leaves its files, for install.sh --skip-build.
# Returns 1, after one line that says why, when it builds from source instead: --from-source, no ready-made build for
# this system, or one that can't be downloaded, doesn't match its checksum or doesn't unpack. With --check it only
# reads SHA256SUMS.
ready_made() {
  if [ "$from_source" = 1 ]; then
    say "$tag is built from source, as --from-source asks."
    return 1
  fi
  machine="$(uname -m)"
  archive=""
  case "$os" in
    Linux)
      system="Linux on $machine"
      if [ "$machine" = x86_64 ]; then archive="hersir-$version-linux-x86_64.tar.gz"; fi ;;
    Darwin)
      system="macOS on $machine"
      # Apple silicon, also when this shell runs under Rosetta
      if [ "$(sysctl -n hw.optional.arm64 2>/dev/null)" = 1 ]; then archive="hersir-$version-macos-arm64.tar.gz"; fi ;;
  esac
  if [ -z "$archive" ]; then
    say "There is no ready-made build of Hersir for $system, so $tag is built from source."
    return 1
  fi
  want=""
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in *"  $archive") want="${line%%  *}" ;; esac
  done <"$tmp/SHA256SUMS"
  case "$want" in *[!0-9a-f]*) want="" ;; esac
  if [ "${#want}" != 64 ]; then
    say "$tag has no ready-made build for $system ($archive isn't in its SHA256SUMS), so it is built from source."
    return 1
  fi
  if [ "$check" = 1 ]; then
    say "$tag has a ready-made build for $system, $archive: the check names what Hersir needs to run it."
    return 0
  fi

  step "Getting the ready-made build of $tag ($archive)"
  get_file "$archive" 900
  case $? in
    1) say "$tag has no $archive to download, so it is built from source."; return 1 ;;
    2) say "Could not download $archive ($(why)), so $tag is built from source."; return 1 ;;
  esac
  got="$(sha256 "$tmp/$archive")" || got=""
  if [ -z "$got" ]; then
    say "Could not work out the checksum of $archive (there is no sha256sum or shasum here), so $tag is built from source."
    return 1
  fi
  if [ "$got" != "$want" ]; then
    say "$archive doesn't match its checksum in SHA256SUMS, so $tag is built from source."
    return 1
  fi
  # Only the files install.sh --skip-build installs, in place of those an earlier build or download left.
  members="target/release/hersir target/release/hersir-mcp"
  if [ "$os" = Darwin ]; then members="$members target/release/bundle/macos/Hersir.app"; fi
  # shellcheck disable=SC2086 # one path each
  (cd "$dir" && rm -rf $members) || { say "Could not make room for the ready-made build in $dir/target, so $tag is built from source."; return 1; }
  # shellcheck disable=SC2086
  if ! tar -xzf "$tmp/$archive" -C "$dir" $members 2>"$tmp/error"; then
    say "Could not unpack $archive ($(sed -n 1p "$tmp/error")), so $tag is built from source."
    return 1
  fi
  say "$archive matches its checksum in SHA256SUMS, and is unpacked into $dir/target/release."
  return 0
}

main() {
  unset CDPATH GIT_DIR GIT_WORK_TREE
  check=0 skip_build=0 build_only=0 uninstall=0 from_source=0 tag=""
  # --from-source is this script's own; the other options go on to install.sh as they are.
  for arg in "$@"; do
    shift
    case "$arg" in
      --from-source) from_source=1; continue ;;
      --check) check=1 ;;
      --skip-build) skip_build=1 ;;
      --build-only) build_only=1 ;;
      --uninstall) uninstall=1 ;;
      -h|--help) usage; return 0 ;;
    esac
    set -- "$@" "$arg"
  done
  # install.sh --check --uninstall would uninstall
  if [ "$check" = 1 ] && [ "$uninstall" = 1 ]; then
    say "--check and --uninstall don't go together (see --help)" >&2
    return 2
  fi
  if [ "$from_source" = 1 ] && [ "$skip_build" = 1 ]; then
    say "--from-source and --skip-build don't go together (see --help)" >&2
    return 2
  fi

  os="$(uname -s)"
  case "$os" in
    Linux|Darwin) ;;
    *) say "This installs Hersir on Linux and macOS. On Windows, run this in PowerShell: irm $GET_PS1 | iex"; return 1 ;;
  esac

  # What this script needs itself: git and bash, and curl to ask for the latest release. On a Mac, /usr/bin/git only
  # offers to install Apple's Command Line Tools until they are there.
  missing=""
  if ! have git; then
    missing="git"
  elif [ "$os" = Darwin ] && [ "$(command -v git)" = /usr/bin/git ] && ! xcode-select -p >/dev/null 2>&1; then
    missing="git"
  fi
  # Each HERSIR_ variable wins over its name from before 0.17.0, GIZAI_, which still works alone.
  repo="${HERSIR_REPO:-${GIZAI_REPO:-$DEFAULT_REPO}}"
  ref="${HERSIR_REF:-${GIZAI_REF:-}}"
  url="${HERSIR_RELEASES_URL:-${GIZAI_RELEASES_URL:-}}"
  builds="${HERSIR_BUILDS_URL:-${GIZAI_BUILDS_URL:-}}"
  have bash || missing="$missing bash"
  [ -n "$ref" ] || have curl || missing="$missing curl"
  if [ "$os" = Darwin ] && [ "$missing" = git ]; then
    say "To install Hersir, first install Apple's Command Line Tools, which bring git: run xcode-select --install, then run this again."
    return 1
  elif [ -n "$missing" ]; then
    # shellcheck disable=SC2086 # one word each
    say "To install Hersir, first install $(words $missing) with your package manager, then run this again."
    return 1
  fi
  # A release is installed only once its signature checks, with ssh-keygen. HERSIR_REF gets no release.
  if [ -z "$ref" ] && ! have ssh-keygen; then
    say "$NO_SSH_KEYGEN"
    return 1
  fi

  # a folder of the repository: as a full path, as git runs in the source folder
  if [ -d "$repo" ]; then repo="$(cd "$repo" && pwd -P)"; fi
  github=""
  case "$repo" in
    https://github.com/*|git@github.com:*|ssh://git@github.com/*)
      github="${repo#*github.com[/:]}"; github="${github%/}"; github="${github%.git}" ;;
  esac
  if [ -z "$ref" ]; then
    if [ -z "$url" ] || [ -z "$builds" ]; then
      if [ -z "$github" ]; then
        say "$repo isn't on GitHub, so there is no latest release to ask it for: set HERSIR_RELEASES_URL and HERSIR_BUILDS_URL, or HERSIR_REF."
        return 1
      fi
      url="${url:-https://api.github.com/repos/$github/releases/latest}"
    fi
    step "Finding Hersir's latest release"
    # As Hersir's own release check: -q first, so your ~/.curlrc plays no part, and redirects to https only.
    answer="$(curl -q --fail --silent --show-error --location --proto '=https,http,file' --proto-redir '=https' \
      --max-time 60 --header 'Accept: application/vnd.github+json' --header 'X-GitHub-Api-Version: 2022-11-28' \
      "$url")" || {
      say "Could not read the latest release from $url. Check your internet connection, then run this again."
      return 1
    }
    tag="$(printf '%s\n' "$answer" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
    if ! printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
      say "The latest release's tag, ${tag:-missing}, isn't a version like v1.2.3, so there is nothing to install."
      return 1
    fi
    say "Latest release: $tag"
    version="${tag#v}"
    builds="${builds:-https://github.com/$github/releases/download/$tag}"
    builds="${builds%/}"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/hersir-get.XXXXXX")" || { say "Could not make a temporary folder."; return 1; }
    trap 'rm -rf "$tmp"' EXIT
    trap 'rm -rf "$tmp"; exit 1' HUP INT TERM
  fi
  what="${tag:-$ref}"

  share="${XDG_DATA_HOME:-$HOME/.local/share}"
  dir="$share/hersir-src"
  # The source folder from before 0.17.0, gizai-src, becomes hersir-src with its build, so the next build is quicker;
  # with a hersir-src there already, it goes.
  if [ -d "$share/gizai-src" ]; then
    if [ -e "$dir" ] || ! mv "$share/gizai-src" "$dir" 2>/dev/null; then rm -rf "$share/gizai-src"; fi
  fi
  step "Getting $what into $dir"
  mkdir -p "$dir" && dir="$(cd "$dir" && pwd -P)" || { say "Could not make $dir."; return 1; }
  # A .git that git can't use (one cut short) is made again.
  if ! src_git rev-parse --git-dir >/dev/null 2>&1; then
    rm -rf "$dir/.git"
    src_git -c init.defaultBranch=main init --quiet || { say "Could not make a git repository in $dir."; return 1; }
  fi
  # As Hersir's own update: a shallow fetch of the tag alone, checked out as it is, and everything else in the folder
  # goes, except the build's target/, so the next build is quicker. A release is checked out only once its signature
  # checks, as the commit its tag points to: until then the folder stays as it was.
  if [ -n "$ref" ]; then spec="$ref"; else spec="+refs/tags/$tag:refs/tags/$tag"; fi
  src_git fetch --depth 1 --no-tags --force -- "$repo" "$spec" || { say "Could not get $what from $repo."; return 1; }
  if [ -n "$ref" ]; then
    say "$ref is a branch or a commit (HERSIR_REF), not a release: it has no signature to check, and is built from source."
    checkout=FETCH_HEAD
  else
    commit="$(src_git rev-parse --verify --quiet "refs/tags/$tag^{commit}")" || {
      say "Could not find the commit of $tag in $dir."
      return 1
    }
    check_signature || return 1
    checkout="$commit"
  fi
  src_git checkout --quiet --force --detach "$checkout" && src_git clean -ffdxq --exclude=/target/ || {
    say "Could not put $what in place in $dir."
    return 1
  }
  if [ ! -f "$dir/install.sh" ]; then
    say "$what has no install.sh, so it can't be installed this way."
    return 1
  fi

  # The release's ready-made build, installed with install.sh --skip-build. Not for --uninstall, nor for your own
  # --skip-build (what is built in the source folder already) or --build-only (a build).
  if [ -z "$ref" ] && [ "$uninstall" = 0 ] && [ "$skip_build" = 0 ] && [ "$build_only" = 0 ] && ready_made; then
    set -- --skip-build "$@"
  fi

  # The release's check first, which says what is missing. Not before --uninstall: install.sh --check --uninstall
  # would uninstall. What it finds missing (1) stops the install, also for a ready-made build, except with your own
  # --skip-build: install.sh installs what is built then too. Any other failure (2, an unknown option) stops it.
  if [ "$uninstall" = 0 ]; then
    bash "$dir/install.sh" --check "$@" </dev/null
    status=$?
    if [ "$check" = 1 ] || { [ "$status" != 0 ] && [ "$status" != 1 ]; }; then return "$status"; fi
    if [ "$status" = 1 ] && [ "$skip_build" = 0 ]; then
      say ""
      say "Install what is missing above, then run this again."
      return 1
    fi
  fi
  bash "$dir/install.sh" "$@" </dev/null
}

main "$@"
