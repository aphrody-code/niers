// Barre de progression linéaire — design/API portés de `var/spaceui/packages/primitives/ProgressBar.tsx`
// (spacedrive), ré-implémentés sur `@base-ui/react/progress` (déjà la brique Radix-like du projet,
// cf. tabs.tsx/switch.tsx/slider.tsx) plutôt que d'ajouter `@radix-ui/react-progress` en double
// emploi. `null` (au lieu de spacedrive `pending`) = indéterminé, valeur native de base-ui.
import { Progress as ProgressPrimitive } from "@base-ui/react/progress"

import { cn } from "@/lib/utils"

function Progress({
  className,
  value,
  ...props
}: ProgressPrimitive.Root.Props) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      value={value}
      className={cn("w-full", className)}
      {...props}
    >
      <ProgressPrimitive.Track
        data-slot="progress-track"
        className={cn(
          "relative h-1 w-full overflow-hidden rounded-full bg-surface-container-highest",
          value == null && "animate-pulse"
        )}
      >
        <ProgressPrimitive.Indicator
          data-slot="progress-indicator"
          className="h-full bg-primary transition-[width] duration-500 ease-in-out data-indeterminate:w-1/3 data-indeterminate:animate-[progress-indeterminate_1.2s_ease-in-out_infinite]"
        />
      </ProgressPrimitive.Track>
    </ProgressPrimitive.Root>
  )
}

export { Progress }
