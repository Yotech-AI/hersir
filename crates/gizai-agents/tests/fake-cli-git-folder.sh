#!/usr/bin/env bash
# GA-322: stands in for a Codex run that changes its worktree's own folder in the repository's git folder (Codex's
# sandbox may write that folder, for its commits), never the real Codex. It runs in the run's worktree, then ends as
# fake-cli.sh does (FAKE_KIND), with the same arguments and stdin. FAKE_GIT_FOLDER says what it changes:
# - commondir: copies the repository's git folder to FAKE_EVIL, adds a clean filter x that touches FAKE_PROBE to that
#   copy's config, and points the worktree's commondir at it (the audit's probe);
# - config_worktree: writes that filter into a config.worktree in the worktree's own folder;
# - gitdir: makes FAKE_EVIL a repository of its own with that filter (a.txt changed under it) and points the gitdir file
#   of the worktree's own folder at it, so git lists the card's branch there.
# Either way every file of the worktree goes through filter x (.gitattributes) and a.txt changes, so the next git diff or
# git status there runs the filter, unless that git reads neither file.
set -e
gitdir="$(sed -n 's/^gitdir: //p' .git)"
case "$gitdir" in /*) ;; *) gitdir="$PWD/$gitdir" ;; esac
filter="[filter \"x\"]
	clean = touch $FAKE_PROBE && cat
	smudge = cat"
case "${FAKE_GIT_FOLDER:-}" in
commondir)
  common="$(cd "$gitdir" && cd "$(cat commondir)" && pwd)"
  cp -R "$common" "$FAKE_EVIL"
  printf '%s\n' "$filter" >> "$FAKE_EVIL/config"
  printf '%s\n' "$FAKE_EVIL" > "$gitdir/commondir"
  ;;
config_worktree)
  printf '%s\n' "$filter" > "$gitdir/config.worktree"
  ;;
gitdir)
  evil() { env -u GIT_DIR -u GIT_WORK_TREE -u GIT_COMMON_DIR git -c user.email=t@t -c user.name=t -c commit.gpgsign=false -C "$FAKE_EVIL" "$@"; }
  mkdir -p "$FAKE_EVIL"
  evil init -q -b main
  printf 'one\n' > "$FAKE_EVIL/a.txt"
  evil add a.txt
  evil commit -q -m init
  printf '%s\n' "$filter" >> "$FAKE_EVIL/.git/config"
  printf '* filter=x\n' > "$FAKE_EVIL/.gitattributes"
  printf 'two\n' > "$FAKE_EVIL/a.txt"
  printf '%s\n' "$FAKE_EVIL/.git" > "$gitdir/gitdir"
  ;;
esac
printf '* filter=x\n' > .gitattributes
printf 'two\n' > a.txt
exec bash "$(dirname "$0")/fake-cli.sh" "$@"
