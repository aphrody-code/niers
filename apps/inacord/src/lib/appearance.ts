// Applique en direct les préférences d'apparence (taille de police, zoom, variante de palette)
// sur le document.
// `fontScale` pilote `html { font-size }` (tout Tailwind est en rem → tout suit).
// `uiZoom` utilise la propriété CSS `zoom` (Chromium/WebView2 — la cible Windows de niers).
import { useEffect } from "react";
import { useTheme } from "next-themes";
import { ACCENT_THEMES, useSettings } from "@/lib/settings";

const BASE_FONT_SIZE_PX = 16;

/** Classes CSS des variantes de palette (cf. styles.css, portage de
 * `var/spaceui/packages/tokens/src/css/themes/*.css`). `spacedrive` = palette de base
 * (`theme.css`), donc AUCUNE classe à poser — d'où la chaîne vide. */
const THEME_CLASS: Record<(typeof ACCENT_THEMES)[number], string> = {
  spacedrive: "",
  midnight: "midnight-theme",
  noir: "noir-theme",
  slate: "slate-theme",
  nord: "nord-theme",
  mocha: "mocha-theme",
};

export function useApplyAppearance(): void {
  const { fontScale, uiZoom, accentTheme } = useSettings();
  const { resolvedTheme } = useTheme();

  useEffect(() => {
    document.documentElement.style.fontSize = `${BASE_FONT_SIZE_PX * fontScale}px`;
  }, [fontScale]);

  useEffect(() => {
    // `zoom` n'est pas dans le typage CSSStyleDeclaration standard de TS.
    (document.body.style as unknown as { zoom: string }).zoom = String(uiZoom);
  }, [uiZoom]);

  // Variante de palette — toutes celles de spaceui sont SOMBRES (`themes/light.css` est la seule
  // palette claire fournie et n'a pas de déclinaison). En thème clair on ne pose donc aucune
  // classe de variante : la poser quand même écraserait `.light` (même spécificité, déclarée
  // avant) et rendrait l'app sombre alors que l'utilisatrice a demandé du clair.
  useEffect(() => {
    const root = document.documentElement;
    for (const cls of Object.values(THEME_CLASS)) if (cls) root.classList.remove(cls);
    if (resolvedTheme === "dark") {
      const cls = THEME_CLASS[accentTheme];
      if (cls) root.classList.add(cls);
    }
  }, [accentTheme, resolvedTheme]);
}
