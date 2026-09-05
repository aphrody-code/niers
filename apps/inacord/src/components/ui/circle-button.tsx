// Portage littéral de `var/spaceui/packages/primitives/src/CircleButton.tsx` — bouton rond des
// barres d'outils/pieds de barre latérale de spacedrive. Seule adaptation : l'icône est passée
// par `name` (vocabulaire `components/ui/Icon.tsx`, SVG lucide embarqué) au lieu d'un composant
// `@phosphor-icons/react`, qui n'est pas une dépendance de ce projet.
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "react";

import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

export const circleButtonStyles = cva(
  [
    "flex items-center justify-center",
    "backdrop-blur-xl transition-[background-color,border-color,color,transform]",
    "border border-app-line/50",
    "rounded-full",
    "active:scale-95",
    "disabled:pointer-events-none disabled:opacity-50",
  ],
  {
    variants: {
      variant: {
        default: "bg-app-overlay/80 text-sidebar-ink-dull hover:bg-app-box hover:text-sidebar-ink",
        active: "bg-app-overlay text-sidebar-ink",
        accent: "border-accent/30 bg-accent/20 text-accent",
        solid: "border-app-line bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink",
      },
      size: {
        sm: "h-7 w-7",
        md: "h-8 w-8",
        lg: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "md",
    },
  },
);

export interface CircleButtonProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "size">,
    VariantProps<typeof circleButtonStyles> {
  /** Nom d'icône (`components/ui/Icon.tsx`). */
  icon?: string;
  /** Rend la variante `active` (survolé/ouvert) — équivalent du `active` amont. */
  active?: boolean;
}

export const CircleButton = forwardRef<HTMLButtonElement, CircleButtonProps>(
  ({ icon, active, variant, size, className, children, ...props }, ref) => (
    <button
      ref={ref}
      type="button"
      className={cn(
        circleButtonStyles({ variant: variant ?? (active ? "active" : "default"), size }),
        children && "w-auto gap-2 px-3",
        className,
      )}
      {...props}
    >
      {icon && <Icon name={icon} size={18} />}
      {children && <span className="text-xs font-medium">{children}</span>}
    </button>
  ),
);

CircleButton.displayName = "CircleButton";
