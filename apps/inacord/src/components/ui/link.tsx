// Remplacement local de `next/link` — même raison que `ui/image.tsx` : les composants portés du
// wiki (`components/wiki/*`) pointent vers des routes web (`/chara/c01000010`, `/skill/whs00340`).
//
// Une application de bureau n'a pas de routeur d'URL. Un `href` est donc traité comme une
// INTENTION de navigation, remise à l'hôte : `NavigationWiki` reçoit la route, à charge pour la
// vue qui monte ces cartes d'ouvrir l'entité correspondante (encyclopédie, éditeur de propriétés,
// explorateur de fichiers). Sans fournisseur, le lien reste inerte plutôt que d'ouvrir un
// navigateur externe sur une page qui n'existe pas hors ligne.
import { createContext, useContext, type AnchorHTMLAttributes, type ReactNode } from "react";

import { cn } from "@/lib/utils";

/** Route du wiki telle qu'un composant porté l'écrit : `/chara/<code>`, `/skill/<code>`, … */
export type RouteWiki = string;

const Navigation = createContext<((route: RouteWiki) => void) | null>(null);

/** Fournit la cible de navigation aux cartes du wiki montées en dessous. */
export function NavigationWiki({
  onNavigate,
  children,
}: {
  onNavigate: (route: RouteWiki) => void;
  children: ReactNode;
}) {
  return <Navigation.Provider value={onNavigate}>{children}</Navigation.Provider>;
}

/** Découpe une route du wiki en `(famille, identifiant)` — `/chara/c01000010` → `chara`, `c01000010`. */
export function decouperRoute(route: RouteWiki): { famille: string; id: string } {
  const parts = route.replace(/^\//, "").split("/");
  return { famille: parts[0] ?? "", id: parts.slice(1).join("/") };
}

export interface LinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> {
  href: RouteWiki;
  prefetch?: boolean;
  replace?: boolean;
  scroll?: boolean;
}

export function Link({ href, className, children, onClick, prefetch: _p, replace: _r, scroll: _s, ...rest }: LinkProps) {
  const naviguer = useContext(Navigation);
  return (
    <a
      href={href}
      className={cn(naviguer ? "cursor-pointer" : "cursor-default", className)}
      onClick={(e) => {
        // Jamais de navigation de la WebView : elle quitterait l'application sans retour.
        e.preventDefault();
        onClick?.(e);
        naviguer?.(href);
      }}
      {...rest}
    >
      {children}
    </a>
  );
}

export default Link;
