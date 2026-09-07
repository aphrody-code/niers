# `mainmenu01` — Mesures & Analyse Visuelle

Pour l'implémentation complète du rendu pixel-perfect du moteur, voir [DESIGN.md](DESIGN.md).

## Synthèse des mesures d'angle et de géométrie (2026-09-06)
- **Angle des tuiles de la rangée** : pente mesurée à **dx/dy = -0,400** (angle exact -21,80°, R² = 1,000).
- **Panneau droit** : pente mesurée à **dx/dy = -0,546** (angle -28,63°, R² = 1,000).
- **Palette mesurée sur capture 2048×1159** :
  - Fond dominant (69,0%) : `#F9FDF9` (Oklch 0,990 0,007 145°)
  - Bleu bandeau (10,4%) : `#93D3F0` (Oklch 0,834 0,077 228°)
  - Bleu nuit tuiles (7,7%) : `#2C497C` (Oklch 0,409 0,093 261°)
  - Bleu icônes (7,1%) : `#4B8DD5` (Oklch 0,633 0,128 252°)

Script d'extraction : `scripts/validation/mesurer-mainmenu.py`.
Valeurs figées dans `packages/inacord-ui/src/shell/geometrie-mainmenu.ts`.
