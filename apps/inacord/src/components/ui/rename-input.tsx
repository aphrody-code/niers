// Édition de nom en ligne (double-clic → input → Entrée valide/Échap annule/perte de focus
// annule, comme le Finder macOS) — logique portée de
// `var/spaceui/packages/explorer/src/RenameInput.tsx` (spacedrive), simplifiée : niers n'a pas
// le concept d'« extension affichée séparément » de spacedrive (utilisé ici pour renommer un mod,
// cf. ModsView.tsx — pas un fichier VFS, qui reste en lecture seule).
import { useCallback, useEffect, useRef, useState } from "react"

import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

export interface RenameInputProps {
  /** Nom courant. */
  name: string;
  /** Appelé avec le nouveau nom (déjà `trim()`é, non-vide, différent de `name`) pour valider. */
  onSave: (newName: string) => Promise<void>;
  /** Appelé sur annulation (Échap, perte de focus, ou nom inchangé/vide). */
  onCancel: () => void;
  className?: string;
}

export function RenameInput({ name, onSave, onCancel, className }: RenameInputProps) {
  const [value, setValue] = useState(name);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const handleSave = useCallback(async () => {
    if (saving) return;
    const trimmed = value.trim();
    if (!trimmed || trimmed === name) {
      onCancel();
      return;
    }
    setSaving(true);
    try {
      await onSave(trimmed);
    } catch {
      setSaving(false);
    }
  }, [value, saving, name, onSave, onCancel]);

  return (
    <Input
      ref={inputRef}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          void handleSave();
        } else if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          onCancel();
        }
      }}
      onBlur={() => {
        if (!saving) onCancel();
      }}
      onClick={(e) => e.stopPropagation()}
      disabled={saving}
      className={cn("h-auto px-1 py-0.5", saving && "opacity-50", className)}
    />
  );
}
