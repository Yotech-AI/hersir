#!/usr/bin/env bash
# GA-350 (GA-326 #1): stands in for a run that commits a nested repository into its worktree, never a real CLI. It runs
# in the run's worktree, then ends as fake-cli.sh does (FAKE_KIND), with the same arguments and stdin:
# - changes a.txt (the card's own change) and makes nested/, a repository of its own with n.txt, whose .gitattributes
#   sends every file through filter x;
# - commits both in the worktree: nested/ goes in as a gitlink, as git adds an embedded repository;
# - then gives nested/'s own config a clean filter x that touches FAKE_PROBE, and changes n.txt, so any git that looks
#   inside nested/ (a plain git status in the worktree does) runs the filter, as you.
set -e
g() {
  env -u GIT_DIR -u GIT_WORK_TREE -u GIT_COMMON_DIR -u GIT_INDEX_FILE \
    git -c user.email=t@t -c user.name=t -c commit.gpgsign=false -c core.hooksPath=/dev/null -c advice.addEmbeddedRepo=false "$@"
}
printf 'two\n' > a.txt
mkdir -p nested
g -C nested init -q -b main
printf 'one\n' > nested/n.txt
printf '* filter=x\n' > nested/.gitattributes
g -C nested add n.txt .gitattributes
g -C nested commit -q -m nested
g add a.txt nested
g commit -q -m "a nested repository"
printf '[filter "x"]\n\tclean = touch %s && cat\n\tsmudge = cat\n' "$FAKE_PROBE" >> nested/.git/config
printf 'two\n' > nested/n.txt
exec bash "$(dirname "$0")/fake-cli.sh" "$@"
