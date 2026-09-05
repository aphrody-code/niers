// Badge de raccourci clavier (« Ctrl+K », « F2 »…) — porté de
// `var/spaceui/packages/primitives/Shortcut.tsx` (spacedrive). Pur affichage, pas de primitive
// base-ui nécessaire.
import type { ComponentProps } from "react"

import { cn } from "@/lib/utils"

export interface ShortcutProps extends Omit<ComponentProps<"kbd">, "children"> {
  /** Texte du raccourci affiché (ex. `"Ctrl+K"`, `"↵"`) — même API que
   * `spaceui/primitives/Shortcut.tsx` (prop `chars`, pas `children`). */
  chars: string;
}

function Shortcut({ className, chars, ...props }: ShortcutProps) {
  return (
    <kbd
      data-slot="shortcut"
      className={cn(
        "inline-flex items-center justify-center rounded-md border border-b-2 border-app-line px-1.5 py-0.5",
        "type-label-small font-medium text-on-surface-variant",
        className
      )}
      {...props}
    >
      {chars}
    </kbd>
  )
}

export { Shortcut }
