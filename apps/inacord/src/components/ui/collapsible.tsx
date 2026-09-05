// Section repliable — design/API porté de `var/spaceui/packages/primitives/Collapsible.tsx`
// (spacedrive), ré-implémenté sur `@base-ui/react/collapsible` plutôt que
// `@radix-ui/react-collapsible`. Sert aux groupes de la barre latérale de l'Explorateur
// (Épinglés/Récents, cf. ExplorerView.tsx `PlacesSidebar`).
import { Collapsible as CollapsiblePrimitive } from "@base-ui/react/collapsible"

import { cn } from "@/lib/utils"

const Collapsible = CollapsiblePrimitive.Root

function CollapsibleTrigger({ className, ...props }: CollapsiblePrimitive.Trigger.Props) {
  return (
    <CollapsiblePrimitive.Trigger
      data-slot="collapsible-trigger"
      className={cn(
        "state-layer group flex w-full items-center justify-between rounded-lg px-2 py-1 text-left",
        className
      )}
      {...props}
    />
  )
}

function CollapsiblePanel({ className, ...props }: CollapsiblePrimitive.Panel.Props) {
  return (
    <CollapsiblePrimitive.Panel
      data-slot="collapsible-panel"
      className={cn(
        "overflow-hidden transition-[height] duration-200 ease-out data-[starting-style]:h-0 data-[ending-style]:h-0",
        className
      )}
      {...props}
    />
  )
}

export { Collapsible, CollapsibleTrigger, CollapsiblePanel }
