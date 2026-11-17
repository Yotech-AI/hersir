import { useEffect, useState } from "react";
import { FolderOpen } from "lucide-react";
import { getLogsInfo, openLogsFolder } from "../api";
import { Field } from "./Form";
import { LOGS_HINT } from "../lib/logs";

/** Settings → General → Data → Hersir's logs (GA-282): the folder of Hersir's own log (GA-281), one file per start, with
 *  Open folder. Until Hersir answers, the folder is `<data folder>/logs`, where it always is. */
export function LogsSetting({ dataDir }: { dataDir: string }) {
  const [folder, setFolder] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    (async () => {
      const info = await getLogsInfo();
      if (live && info?.folder) setFolder(info.folder);
    })().catch(() => {});
    return () => { live = false; };
  }, []);
  const open = async () => {
    setErr(null);
    try { await openLogsFolder(); } catch (e) { setErr(String(e)); }
  };
  return (
    <Field label="Hersir's logs" wide hint={LOGS_HINT} error={err}>
      <div className="input-group logs-folder"><span className="mono bk-folder">{folder ?? `${dataDir}/logs`}</span>
        <button type="button" className="btn sm" onClick={open}><FolderOpen className="icon" />Open folder</button></div>
    </Field>
  );
}
