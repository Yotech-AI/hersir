#!/usr/bin/env bash
# GA-322: stands in for Gemini CLI 0.65 reading the .env of the folder it works in, never the real Gemini, then ends as
# fake-cli.sh does (FAKE_KIND=gemini), with the same arguments and stdin. What Gemini 0.65 does (loadEnvironment and
# findEnvFile in its bundle): unless its system settings (GEMINI_CLI_SYSTEM_SETTINGS_PATH, else
# /etc/gemini-cli/settings.json) have advanced.ignoreLocalEnv on, it sets each variable of ./.env that isn't set yet
# (`Object.hasOwn(process.env, key)`: an empty one is set), once it trusts the folder. This one then runs a command the
# way Gemini's shell tool does (bash -c), so a BASH_ENV from the .env runs first. It writes to stderr:
#   gemini settings: <file> ignoreLocalEnv=<true|false>
#   gemini settings text: <the file's text on one line>
#   loaded from .env:<the names it set>
#   BASH_ENV=[…] ENV=[…] GOOGLE_GEMINI_BASE_URL=[…] GOOGLE_VERTEX_BASE_URL=[…]   ("unset" when not set)
#   gemini defaults: <GEMINI_CLI_SYSTEM_DEFAULTS_PATH, or unset>
settings="${GEMINI_CLI_SYSTEM_SETTINGS_PATH:-/etc/gemini-cli/settings.json}"
ignore="$(python3 -c '
import json, sys
try:
    s = json.load(open(sys.argv[1]))
except FileNotFoundError:
    s = {}
a = s.get("advanced") if isinstance(s, dict) else None
print("true" if isinstance(a, dict) and a.get("ignoreLocalEnv") is True else "false")
' "$settings")"
loaded=""
if [ "$ignore" != true ] && [ -f .env ]; then
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in ''|'#'*) continue ;; esac
    k="${line%%=*}"
    v="${line#*=}"
    if ! printenv "$k" >/dev/null; then
      export "$k=$v"
      loaded="$loaded $k"
    fi
  done < .env
fi
echo "gemini settings: $settings ignoreLocalEnv=$ignore" >&2
echo "gemini settings text: $(tr -d '\n' < "$settings" 2>/dev/null)" >&2
echo "loaded from .env:$loaded" >&2
echo "BASH_ENV=[${BASH_ENV-unset}] ENV=[${ENV-unset}] GOOGLE_GEMINI_BASE_URL=[${GOOGLE_GEMINI_BASE_URL-unset}] GOOGLE_VERTEX_BASE_URL=[${GOOGLE_VERTEX_BASE_URL-unset}]" >&2
echo "gemini defaults: ${GEMINI_CLI_SYSTEM_DEFAULTS_PATH-unset}" >&2
bash -c 'true'
exec bash "$(dirname "$0")/fake-cli.sh" "$@"
