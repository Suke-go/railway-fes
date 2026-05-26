// Done screen — session ended. Reset to idle and clear cached session info.

import { setState } from "../state";
import { reset as resetSession } from "../session";

export function wireDoneScreen(): void {
  document.getElementById("btn-reset")?.addEventListener("click", () => {
    resetSession();
    setState("idle");
  });
}
