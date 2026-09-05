// Popover flottant — design porté de `var/spaceui/packages/primitives/Popover.tsx` (spacedrive :
// panneau arrondi `bg-app-overlay`/`border-app-line`), ré-implémenté sur `@base-ui/react/popover`
// (déjà utilisé par le projet) au lieu de `@radix-ui/react-popover`. Sert notamment au bouton
// « Options d'affichage » de l'Explorateur (cf. ExplorerView.tsx).
import { Popover as PopoverPrimitive } from "@base-ui/react/popover"

import { cn } from "@/lib/utils"

const Popover = PopoverPrimitive.Root
const PopoverTrigger = PopoverPrimitive.Trigger
const PopoverClose = PopoverPrimitive.Close

function PopoverContent({
  className,
  sideOffset = 8,
  align = "start",
  children,
  ...props
}: PopoverPrimitive.Popup.Props & PopoverPrimitive.Positioner.Props) {
  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Positioner sideOffset={sideOffset} align={align} className="z-50" {...props}>
        <PopoverPrimitive.Popup
          data-slot="popover-content"
          className={cn(
            "flex flex-col gap-2 rounded-lg border border-app-line bg-app-overlay p-3",
            "text-ink shadow-lg shadow-black/40 outline-none",
            "origin-[var(--transform-origin)] transition-[transform,opacity] data-starting-style:scale-95 data-starting-style:opacity-0 data-ending-style:scale-95 data-ending-style:opacity-0",
            className
          )}
        >
          {children}
        </PopoverPrimitive.Popup>
      </PopoverPrimitive.Positioner>
    </PopoverPrimitive.Portal>
  )
}

export { Popover, PopoverTrigger, PopoverContent, PopoverClose }
