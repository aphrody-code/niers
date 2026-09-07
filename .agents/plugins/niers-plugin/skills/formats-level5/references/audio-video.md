# Audio, vidéo et synchronisation labiale

| Format | Magic | Extension | Module | Contenu |
|---|---|---|---|---|
| HCA | `HCA\0` (ou chiffré) | `.hca` | `cri_audio.rs` | Audio Criware |
| ACB | (table @UTF) | `.acb` | `cri_audio.rs` | Banque audio |
| AWB | `AFS2` | `.awb` | `cri_audio.rs` | Archive audio |
| USM | `CRID` | `.usm` | — | Vidéo Criware |
| p3lip | `lip\0` | `.p3lip` | `lip.rs` | Visèmes horodatés pour la synchro labiale |

## Décoder de l'audio

Le FFI n'expose **pas** le décodage audio : l'outil MCP `asset_get` avec `decode: "audio"` passe
par le service HTTP `nie-model-serve` (`source: "model-serve"` dans la réponse), qui rend un WAV.
Sans ce service lancé, l'appel échoue proprement — les autres outils continuent de fonctionner.

## p3lip

- Magic `lip\0` — **le module précise explicitement que ce n'est pas autre chose** ; ne pas
  supposer un magic `p3lip`.
- Disposition documentée en tête de `lip.rs` : `0x08` taille du fichier (égale à la longueur
  réelle), `0x14` durée totale en secondes (f32), `0x18` offset de la table 1 (constant à 112),
  `0x1C` constant à 64.
- 20 357 fichiers sous `data/common/sound/<lang>/<event>_<n>.p3lip`, à jouer en synchro avec
  l'`.acb`/`.awb` correspondant.
- `parse` échoue avec `Corrupt` si la taille déclarée est incohérente avec le contenu : c'est un
  contrôle utile, ne pas le contourner.

## HCA chiffré

Certains `.hca` sont chiffrés. Le module gère les deux cas, mais un HCA chiffré non reconnu
donnera du bruit plutôt qu'une erreur — vérifier l'écoute avant de conclure qu'un décodage est
correct.
