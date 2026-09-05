// Groupe de bascules exclusif (ex. vue Liste/Grille de l'Explorateur) — design/API porté de
// `var/spaceui/packages/primitives/ToggleGroup.tsx` (spacedrive), ré-implémenté sur
// `@base-ui/react/toggle-group` + `@base-ui/react/toggle` (déjà la brique du projet) plutôt que
// `@radix-ui/react-toggle-group`.
import { ToggleGroup as ToggleGroupPrimitive } from "@base-ui/react/toggle-group"
import { Toggle as TogglePrimitive } from "@base-ui/react/toggle"

import { cn } from "@/lib/utils"

function ToggleGroup({ className, ...props }: ToggleGroupPrimitive.Props) {
  return (
    <ToggleGroupPrimitive
      data-slot="toggle-group"
      className={cn(
        "inline-flex items-center gap-0.5 rounded-full bg-surface-container-high p-1",
        className
      )}
      {...props}
    />
  )
}

function ToggleGroupItem({ className, ...props }: TogglePrimitive.Props) {
  return (
    <TogglePrimitive
      data-slot="toggle-group-item"
      className={cn(
        "state-layer inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-on-surface-variant",
        "type-label-medium transition-colors data-pressed:bg-surface-container-lowest data-pressed:text-on-surface",
        "disabled:pointer-events-none disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
}

export { ToggleGroup, ToggleGroupItem }
