import { TRUST_ALLOW, TRUST_ASK, TRUST_LABEL, trustWarning } from "../lib/mcp";

/** GA-271: After the Chief uses it, for one of the Chief's sources (an MCP server, the browser, the web): Ask for my OK (the
 * default) or Allow, as two radio buttons (arrow keys move between them), with the warning under Allow. Only in the form of
 * an agent with Chat on. `id` names the group, once in the form; `source` is what a screen reader hears the group is for,
 * like "otus" or "the web"; `name` goes in the warning. */
export function TrustChoice({ id, source, name, trusted, disabled, onChange }: {
  id: string; source: string; name: string; trusted: boolean; disabled?: boolean; onChange: (trusted: boolean) => void;
}) {
  const warn = `${id}-warn`;
  return (
    <div className="ma-trust">
      <div className="ma-trust-choice" role="radiogroup" aria-label={`After the Chief uses ${source}`}>
        <span className="ma-trust-label" aria-hidden="true">{TRUST_LABEL}</span>
        <label className="check"><input type="radio" name={id} value="ask" checked={!trusted} disabled={disabled}
          onChange={() => onChange(false)} /> {TRUST_ASK}</label>
        <label className="check"><input type="radio" name={id} value="allow" checked={trusted} disabled={disabled}
          aria-describedby={trusted ? warn : undefined} onChange={() => onChange(true)} /> {TRUST_ALLOW}</label>
      </div>
      {/* Read out when it shows, as the focus stays on Allow. */}
      <div aria-live="polite">{trusted && <span id={warn} className="warn">{trustWarning(name)}</span>}</div>
    </div>
  );
}
