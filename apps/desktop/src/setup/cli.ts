/** The command line section: link the app onto PATH as `sidepaw`. */
import { invoke } from "@tauri-apps/api/core";
import type { CommandLink } from "../generated/CommandLink";
import { t } from "../i18n";
import { busy, el, note } from "./dom";

function render(link: CommandLink) {
  el("cli-path").textContent = link.path;
  el<HTMLButtonElement>("cli-remove").disabled = !link.installed;
  el<HTMLButtonElement>("cli-install").textContent = t(link.installed ? "cli.update" : "cli.install");
  if (link.installed) note("cli-msg", t(link.on_path ? "cli.ready" : "cli.not_on_path"), !link.on_path);
  else note("cli-msg", "");
}

export async function mountCli() {
  render(await invoke<CommandLink>("command_status"));
  el("cli-install").addEventListener("click", () =>
    busy("cli-msg", async () => render(await invoke<CommandLink>("command_install"))),
  );
  el("cli-remove").addEventListener("click", () =>
    busy("cli-msg", async () => render(await invoke<CommandLink>("command_remove"))),
  );
}
