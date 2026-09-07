---
name: aphrody-pet
description: Charger et valider le pet Codex Aphrody v2 depuis le package canonique avec le runtime Rust nie-aphrody.
---

# Runtime Aphrody v2

Le package Aphrody est un asset 2D Codex, pas un modèle Level-5. Le point d'entrée Rust canonique
est `nie-aphrody` (`crates/engine/nie-aphrody`). Il charge `pet.json`, `animations.json` et
`sprites/spritesheet.png`, puis vérifie la géométrie 8×11, les hashes RGBA, les rectangles et les
statistiques alpha avant toute extraction.

Le paquet validé est vendored dans
`crates/engine/nie-aphrody/assets/aphrody/`. La voie stable est `Pet::bundled()` : elle embarque
les quatre fichiers au build et ne dépend donc ni du répertoire courant ni d'un chemin machine.

## Contrat

- atlas RGBA 1536×2288, 8 colonnes × 11 lignes, cellules 192×208 ;
- 9 animations standard, `look-neutral`, et 16 directions `look-directions` ;
- extraction par `Pet::extract(&frame)` sans rééchantillonnage ;
- direction par `Pet::direction(degrees)` avec distance angulaire circulaire ;
- diagnostic complet par `Pet::diagnose()`, qui doit rendre `ok() == true`.

## Exemple Rust

```rust
use nie_aphrody::Pet;

let pet = Pet::bundled()?;
let frame = pet.animation("idle").and_then(|animation| animation.frames.first()).ok_or("idle absent")?;
let rgba = pet.extract(frame)?;
let gaze = pet.direction(90.0).ok_or("directions absentes")?;
assert!(pet.diagnose().ok());
```

Ne jamais copier ou vendorer les sprites depuis un run non validé : l'import dans un package
canonique attend `qa/run-summary.json` avec `ok: true`. Ne pas confondre ce spritesheet avec
G4TX (texture Level-5), G4MD/G4MG/G4SK (modèle 3D) ou GLB.

## Référence 3D Aphrody

Pour la variante 3D IE1, le code canonique est `c01001900` (`chara_id 0x37D7ACFB`, fiche
primaire `0x9E23A289`). Les sources vérifiées sont le visage
`data/common/chr/_face/01_IE1/c01001900/c01001900.g4md` + `.g4mg`, le squelette partagé
`data/common/chr/c000101/c000101.g4sk`, le corps profil 0
`data/common/chr/_face/20_EDIT/_base/base_normal_00.{g4md,g4mg}`, et la texture visage
`data/dx11/chr/_face/01_IE1/c01001900/c01001900.g4tx`. La tenue Zeus est `u011001`, texture
`u011001_10.g4tx`, kit `0xE17C3465`, CRC fielder `0xF0006501`.

La production 3D passe par `nie-model-serve /model-full/c01001900.glb`. Elle reste distincte du
runtime 2D et doit être considérée bloquée tant que les prérequis réels
`data/dx11/model/base_normal_00.glb`, `data/dx11/model/c01001900.glb` et
`var/uniform-model-map.ndjson` ne sont pas disponibles et inspectés.
