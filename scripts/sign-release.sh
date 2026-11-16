#!/bin/bash
# Signs one of Hersir's releases, on Jeffrey's own computer:   scripts/sign-release.sh vX.Y.Z [--again]
# Run it once Release builds (.github/workflows/release-builds.yml) has uploaded the release's three archives and
# SHA256SUMS (docs/RELEASING.md, step 5). It signs this message, byte for byte, with Hersir's release key: the line
# "hersir vX.Y.Z <commit>" and a newline, where <commit> is the 40-character id of the commit the tag points to,
# followed by the release's SHA256SUMS exactly as uploaded. The signature becomes the release's fifth asset,
# hersir-X.Y.Z.sig, which the one-line install and Update check against the keys of release-signers, built into them,
# before anything of the release runs. Then it starts One-line install (.github/workflows/one-line.yml) again.
#
# - It shows what it signs (the version, the commit with its subject, whether that is production's latest commit, and
#   SHA256SUMS) and signs only after y. Before that it stops, with one line that says why, for a version that isn't
#   vX.Y.Z, and for a release without SHA256SUMS or one of its archives, or with hersir-X.Y.Z.sig already (--again signs
#   it again and replaces it).
# - The key is HERSIR_RELEASE_KEY, by default ~/.ssh/hersir-release. ssh-keygen asks for its passphrase on the
#   terminal: a key without one is refused, and ssh-agent is never used. The private keys never go to GitHub, CI or an
#   agent, and nothing in CI signs.
# - The new signature must check against release-signers before it is uploaded. HERSIR_RELEASE_SIGNERS, a file in the
#   allowed-signers format, replaces release-signers, as it does for the one-line install and Update: for tests, which
#   sign fake releases with a throwaway key, never with Hersir's.
# A bash script for Linux and macOS, also for macOS's own bash 3.2. It needs gh, logged in to GitHub, and ssh-keygen.

set -euo pipefail

# Every program this script runs (ssh-keygen and gh above all; it needs no git) comes from /usr/bin, on macOS also from
# /opt/homebrew/bin, when it is there, before the other folders on PATH: a program of the same name that something put
# in a folder earlier on PATH (~/.local/bin, say) never sees the release, the key or its passphrase. bash itself is
# /bin/bash, by the first line, for the same reason.
case "$OSTYPE" in
  darwin*) PATH="/usr/bin:/opt/homebrew/bin:$PATH" ;;
  *) PATH="/usr/bin:$PATH" ;;
esac
export PATH
# Never ssh-agent, and never an askpass program: the passphrase goes to ssh-keygen itself, on the terminal. Everything
# from GitHub comes from github.com.
unset SSH_AUTH_SOCK SSH_ASKPASS GH_HOST CDPATH
export SSH_ASKPASS_REQUIRE=never

REPO=Yotech-AI/hersir
NAMESPACE=hersir-release
ROOT="$(cd -P "$(dirname "$0")/.." && pwd)"
NL=$'\n'

say() { printf '%s\n' "$*"; }
# One line that says why, to stderr. Nothing is signed or uploaded after it.
fail() { printf '%s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'USAGE'
Signs Hersir's release vX.Y.Z with Hersir's release key, once Release builds has uploaded its archives and SHA256SUMS:
  scripts/sign-release.sh vX.Y.Z           shows what it signs, signs after y, uploads hersir-X.Y.Z.sig to the release
                                           and starts One-line install
  scripts/sign-release.sh vX.Y.Z --again   signs it again and replaces the release's hersir-X.Y.Z.sig
Environment: HERSIR_RELEASE_KEY, the private key's file (default ~/.ssh/hersir-release), which must have a passphrase;
HERSIR_RELEASE_SIGNERS, an allowed-signers file instead of release-signers, for tests with a throwaway key.
It needs gh, logged in to GitHub (gh auth login), and ssh-keygen. docs/RELEASING.md, step 5, says when to run it.
USAGE
}

no_version="Give the release's version, like v1.2.3: scripts/sign-release.sh vX.Y.Z (see --help)."
[ "$#" -gt 0 ] || fail "$no_version"
tag="" again=0
for arg in "$@"; do
  case "$arg" in
    --again) again=1 ;;
    -h|--help) usage; exit 0 ;;
    -*) fail "$arg isn't an option of sign-release.sh (see --help)." ;;
    *) [ -z "$tag" ] || fail "Give one version, not $tag and $arg."; tag="$arg" ;;
  esac
done
[ -n "$tag" ] || fail "$no_version"
VERSION_RE='^v[0-9]+\.[0-9]+\.[0-9]+$'
[[ $tag =~ $VERSION_RE ]] || fail "$tag isn't a version like v1.2.3, so there is nothing to sign."

version="${tag#v}"
sig="hersir-$version.sig"
archives=("hersir-$version-linux-x86_64.tar.gz" "hersir-$version-macos-arm64.tar.gz" "hersir-$version-windows-x86_64.zip")

command -v ssh-keygen >/dev/null || fail "ssh-keygen isn't installed: it comes with OpenSSH."
command -v gh >/dev/null || fail "gh isn't installed: get it from https://cli.github.com, then run gh auth login."
gh auth status --hostname github.com >/dev/null 2>&1 </dev/null ||
  fail "gh isn't logged in to GitHub: run gh auth login, then this again."

# The key: a private key's file with a passphrase. A key that loads with an empty passphrase has none, so anything that
# can read the file could sign with it. (ssh-keygen -Y sign takes a .pub file too, and then signs with the private key
# next to it, which this check wouldn't have seen.)
key="${HERSIR_RELEASE_KEY:-$HOME/.ssh/hersir-release}"
case "$key" in
  *.pub) fail "$key is a public key: HERSIR_RELEASE_KEY names the private key's file." ;;
esac
[ -f "$key" ] || fail "There is no release key at $key: set HERSIR_RELEASE_KEY to its file."
if ssh-keygen -y -P '' -f "$key" >/dev/null 2>&1 </dev/null; then
  fail "The key $key has no passphrase, so it signs nothing: give it one with ssh-keygen -p -f $key."
fi

signers="${HERSIR_RELEASE_SIGNERS:-$ROOT/release-signers}"
[ -f "$signers" ] || fail "There is no $signers to check the signature against."

tmp="$(mktemp -d "${TMPDIR:-/tmp}/hersir-sign.XXXXXX")" || fail "Couldn't make a temp folder."
trap 'rm -rf "$tmp"' EXIT

# gh, which reads nothing from the terminal (the answer and the passphrase are yours), with what it says on a failure
# kept for why. Each call names the release's repository.
github() { gh "$@" </dev/null 2>"$tmp/error"; }
why() {
  local line
  line="$(head -n 1 "$tmp/error" 2>/dev/null || true)"
  printf '%s' "${line:-gh said nothing more}"
}

# The release's assets: its three archives and SHA256SUMS, and its signature only with --again.
assets="$(github release view "$tag" --repo "$REPO" --json assets --jq '.assets[].name')" ||
  fail "Couldn't read the release $tag of $REPO: $(why)"
listed() { case "$NL$assets$NL" in *"$NL$1$NL"*) return 0 ;; esac; return 1; }
for file in "${archives[@]}" SHA256SUMS; do
  listed "$file" ||
    fail "The release $tag has no $file yet: sign it once Release builds has uploaded its builds (gh run list --workflow release-builds.yml)."
done
if listed "$sig" && [ "$again" = 0 ]; then
  fail "The release $tag is signed already ($sig): --again signs it again and replaces it."
fi

# The commit the tag points to, peeled: a release's own tag points to it straight away, an annotated tag through a tag
# object (or a few).
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
  fail "The tag $tag doesn't lead to a commit, so there is nothing to sign."
fi
# Its subject, without control characters: an escape sequence in it could make the terminal show something else.
subject="$(github api "repos/$REPO/git/commits/$commit" --jq '.message | split("\n")[0]')" ||
  fail "Couldn't read the commit $commit: $(why)"
subject="$(printf '%s' "$subject" | LC_ALL=C tr -d '\000-\037\177')"
production="$(github api "repos/$REPO/git/ref/heads/production" --jq '.object.sha')" ||
  fail "Couldn't read production's latest commit: $(why)"
[[ $production =~ $SHA_RE ]] || fail "Couldn't read production's latest commit: GitHub gave no commit id."

github release download "$tag" --repo "$REPO" --pattern SHA256SUMS --dir "$tmp" ||
  fail "Couldn't download the SHA256SUMS of $tag: $(why)"
sums="$tmp/SHA256SUMS"
[ -f "$sums" ] || fail "gh downloaded no SHA256SUMS of $tag."
# SHA256SUMS as Release builds writes it: "<sha256>  <file name>" for each archive, in its order, and nothing else.
# So what the terminal shows below is what gets signed: no other line, and no character a terminal acts on rather than
# shows.
bad_sums="The SHA256SUMS of $tag isn't one line per archive, as Release builds writes it, so it isn't signed."
SUM_RE='^[0-9a-f]{64}  (.+)$'
n=0
while IFS= read -r line || [ -n "$line" ]; do
  if [ "$n" -ge 3 ] || ! [[ $line =~ $SUM_RE ]] || [ "${BASH_REMATCH[1]}" != "${archives[$n]}" ]; then
    fail "$bad_sums"
  fi
  sum_lines[$n]="$line"
  n=$((n + 1))
done < "$sums"
[ "$n" = 3 ] || fail "$bad_sums"

# The message, byte for byte as the one-line install and Update rebuild it. ssh-keygen writes its signature next to it,
# as hersir-X.Y.Z.sig, the asset's name.
message="$tmp/hersir-$version"
{ printf 'hersir %s %s\n' "$tag" "$commit"; cat "$sums"; } > "$message"

say ""
say "Hersir's release $tag ($REPO)"
say "  Version     $tag"
say "  Commit      $commit ${subject:-(no subject)}"
if [ "$commit" = "$production" ]; then
  say "  Production  its latest commit is this one"
else
  say "  Production  its latest commit is $production, not this one"
fi
say "  SHA256SUMS  ${sum_lines[0]}"
say "              ${sum_lines[1]}"
say "              ${sum_lines[2]}"
say "  Key         $key"
if [ -n "${HERSIR_RELEASE_SIGNERS:-}" ]; then
  say "  Checked by  $signers (HERSIR_RELEASE_SIGNERS), not release-signers"
fi
if listed "$sig"; then
  say "  Replaces    the release's $sig (--again)"
fi
printf 'Sign %s? [y/N] ' "$tag"
answer=""
read -r answer || true
[ "$answer" = y ] || fail "Nothing is signed: the answer wasn't y."

# ssh-keygen asks for the key's passphrase itself (SSH_AUTH_SOCK is unset above, so ssh-agent is out).
ssh-keygen -Y sign -f "$key" -n "$NAMESPACE" "$message" ||
  fail "ssh-keygen didn't sign $tag with $key (a wrong passphrase?), so nothing is uploaded."
ssh-keygen -Y verify -f "$signers" -I "$NAMESPACE" -n "$NAMESPACE" -s "$message.sig" <"$message" >/dev/null 2>&1 ||
  fail "The new signature doesn't check against $signers, so it isn't uploaded: is $key one of its keys?"

clobber=""
if [ "$again" = 1 ]; then clobber=--clobber; fi
github release upload "$tag" "$message.sig" --repo "$REPO" ${clobber:+"$clobber"} ||
  fail "Couldn't upload $sig to the release $tag: $(why)"
assets="$(github release view "$tag" --repo "$REPO" --json assets --jq '.assets[].name')" ||
  fail "Uploaded $sig, but couldn't read the release $tag again: $(why)"
listed "$sig" || fail "Uploaded $sig, but the release $tag doesn't list it: see gh release view $tag --repo $REPO."

# One-line install installs the latest release with hersir.dev's own lines: now with its signature.
github workflow run one-line.yml --repo "$REPO" ||
  fail "Signed $tag, but One-line install didn't start: $(why). Start it with gh workflow run one-line.yml --repo $REPO."
say "Signed $tag: the release has $sig, and One-line install runs (gh run list --workflow one-line.yml --repo $REPO)."
