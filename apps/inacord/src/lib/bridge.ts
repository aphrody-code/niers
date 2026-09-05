// Pont de contrôle : laisse le serveur MCP `nie-mcp` piloter cette fenêtre (naviguer, ouvrir
// un asset, changer d'onglet, notifier). Le protocole vient de `@niers/bridge`, importé tel
// quel par les deux bouts — la même union `BridgeCommand` sert ici et côté serveur, donc une
// commande ajoutée sans être traitée casse la compilation au lieu de partir en silence.
//
// Le pont est opportuniste et facultatif : sans serveur MCP en écoute, `connectBridge`
// réessaie en arrière-plan et l'explorateur fonctionne normalement.
import { useEffect, useRef, useState } from "react";
import { connectBridge, type BridgeHandlers } from "@niers/bridge";
import { getSettings } from "@/lib/settings";

/** Version annoncée au serveur — celle du `package.json` de l'application. */
const APP_VERSION = "0.4.0";

/**
 * Branche la fenêtre sur le pont tant que le composant est monté.
 *
 * Les handlers sont lus dans une ref à chaque commande : le socket n'est pas recréé quand
 * l'état de l'application change, ce qui éviterait sinon une reconnexion à chaque frappe.
 *
 * @returns `true` tant qu'un serveur MCP est connecté.
 */
export function useBridge(handlers: BridgeHandlers): boolean {
  const [connected, setConnected] = useState(false);
  const ref = useRef(handlers);
  ref.current = handlers;

  useEffect(() => {
    if (getSettings().bridgeEnabled === false) return;

    const client = connectBridge(
      {
        getState: () => ref.current.getState(),
        navigate: (prefix, select) => ref.current.navigate(prefix, select),
        open: (path) => ref.current.open(path),
        setTab: (tab) => ref.current.setTab(tab),
        toast: (message, kind) => ref.current.toast(message, kind),
      },
      { app: "nie-explorer", version: APP_VERSION, onStatus: setConnected },
    );
    return () => client.close();
  }, []);

  return connected;
}
