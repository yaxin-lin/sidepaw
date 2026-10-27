/** The "Sent to the model" list: every remote call, newest first. */
import { invoke } from "@tauri-apps/api/core";
import { PROVIDERS } from "../brain/llm";
import type { AuditRow } from "../generated/AuditRow";
import { t, type Key } from "../i18n";
import { el } from "./dom";

export async function renderAudit() {
  const rows = await invoke<AuditRow[]>("audit_recent", { limit: 20 });
  const body = el<HTMLTableSectionElement>("audit-rows");
  body.replaceChildren();
  el("audit-empty").hidden = rows.length > 0;
  for (const r of rows) {
    const tr = document.createElement("tr");
    for (const [text, mono] of [
      [r.ts.replace("T", " ").slice(0, 19), true],
      [PROVIDERS[r.provider]?.label ?? r.provider, false],
      [t(`purpose.${r.purpose}` as Key), false],
      [`${r.bytes} B`, true],
      [r.sha256.slice(0, 12), true],
    ] as const) {
      const td = document.createElement("td");
      if (mono) td.className = "mono";
      td.textContent = text;
      tr.append(td);
    }
    body.append(tr);
  }
}
