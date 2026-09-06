# Domaine 4 — Audio & vidéo (32 369 fichiers)

Cartographie du sous-arbre `data/common/sound/`, `data/common/sound_asset/`,
`data/common/movie/`, `data/dx11/movie/` du VFS d'*Inazuma Eleven: Victory Road*, en vue des
routes Aphrody qui doivent le servir **comme `nie.exe`** : par cue, pas par fichier physique.
Inventaire source : `var/vfs/lot4-audio.txt` (`chemin taille [cpk]`).

## 1. Les chiffres

```
wc -l var/vfs/lot4-audio.txt                    → 32 369
awk '{s+=$2} END{print s/1024/1024/1024}' …      → 27,24 Gio (poids brut du domaine sur le disque)
```

Ventilation par extension (`awk -F/ ... | sort | uniq -c` sur l'inventaire) :

| Extension | Fichiers | Octets | Gio |
|---|---:|---:|---:|
| `.p3lip` (lip-sync) | 21 047 | 9 269 952 | 0,009 |
| `.awb` (banque, données brutes) | 5 512 | 8 254 584 984 | **7,688** |
| `.acb` (cue sheet) | 5 512 | 110 068 256 | **0,103** |
| `.usm` (cinématique) | 194 | 20 869 432 608 | 19,436 |
| `.bin` (résidus, non identifiés ici) | 103 | 2 566 718 | 0,002 |
| `.acf` | 1 | 15 072 | 0,000 |

Ventilation par sous-dossier (`awk -F/ '{print $1"/"$2"/"$3}' | uniq -c`) :

| Racine | Fichiers |
|---|---:|
| `data/common/sound/` | 21 150 (quasi tout en `.p3lip`, + quelques `.cfg.bin` de configuration commentateur/BGM) |
| `data/common/sound_asset/` | 11 025 (les paires `.acb`/`.awb`, en `fr/en/ja/de/…` pour les voix) |
| `data/common/movie/` | 97 (`.usm`, poids logique) |
| `data/dx11/movie/` | 97 (les mêmes 97 films, copie DX11 — même paire de comptes que ci-dessus, à traiter comme jumeaux, cf. §8) |

**Le rapport 7,688 Gio (AWB) contre 0,103 Gio (ACB) — 74× — est LE fait central de ce domaine**,
détaillé au §3.

## 2. Grammaire des chemins et appariement `.acb` ↔ `.awb`

- Musique et effets : `data/common/sound_asset/<nom>.acb` + `data/common/sound_asset/<nom>.awb`
  — même radical, extension jumelle. Exemples de l'inventaire :
  - `bgm.acb` (239 744 o) / `bgm.awb` (864 948 904 o)
  - `bgm_chronicle.acb` (308 672 o) / `bgm_chronicle.awb` (**1 354 646 228 o = 1 291,9 Mio**,
    la plus grosse banque du jeu)
  - `bgm_title.acb` (7 392 o) / `bgm_title.awb` (2 020 290 o)
  - `anime_stream.acb` / `anime_stream.awb` (654 480 330 o)
  - `waza_stream.acb` / `waza_stream.awb` (291 499 904 o)
- Voix localisées : `data/common/sound_asset/<langue>/<nom>.acb`, ex.
  `data/common/sound_asset/ja/anime_stream_voice.awb` (285 611 596 o),
  `data/common/sound_asset/ja/partvoice_sub2.acb` (881 952 o),
  `data/common/sound_asset/en/partvoice_sub2.acb` (871 392 o).
- Cinématiques : `data/common/movie/<Nom>.usm` et `data/dx11/movie/<Nom>.usm` — **mêmes noms dans
  les deux racines**, tailles légèrement différentes (ex. `Chronicle_Title_EN_01.usm` :
  2 278 112 o côté `common`, 8 868 352 o côté `dx11` — pas un doublon strict, deux encodages).
  `IE_15th.usm` et `L5logo.usm` sont **loose** (`[<loose>]`, hors CPK).
- Lip-sync : `data/common/sound/<langue>/<radical>.p3lip`, radical partagé avec l'événement de
  dialogue (`ev01_00300_010_010.p3lip`) — nom d'écran/scène, pas de cue.
- Configuration audio : `data/common/sound/*_0.0X.XX.cfg.bin` — `bgm_config`, `caster_set_*`
  (déclencheurs de commentaire), `*_sequence_se_config` — hors binaire audio, ce sont des T2B/RDBN
  du domaine 1 (config), pas des sons.

**Appariement vérifié par le code, pas supposé** — `crates/engine/nie-formats/src/cri_audio.rs:572`
(`acb_stream_awb_header`) et `nie-model-serve` (`awb_frere`, `crates/tools/nie-model-serve/src/main.rs:2562`)
résolvent l'AWB frère par substitution d'extension sur le même radical, **avec repli sur l'AWB
embarqué** (`embeddedAwb`) quand la banque n'a pas d'externe.

## 3. Le point central : `.awb` n'est pas une piste, c'est une banque

Un `.awb` (AFS2 Wave Bank) est une archive indexée par **cue-id**, pas par piste nommée :
`bgm_chronicle.awb` pèse 1 291,9 Mio à lui seul et ne porte aucun nom lisible en son sein — le nom
vit dans l'`.acb` frère (Cue Sheet, format `@UTF` de CRIWARE).

Chaîne réelle : **ACB → CueTable → (CueId, ReferenceIndex) → CueNameTable (nom) → AWB[cue-id]
(les octets HCA/ADX)**. `nie_formats::cri_audio::acb_cues`
(`crates/engine/nie-formats/src/cri_audio.rs:641`) résout tout ça **sans jamais ouvrir l'AWB** —
c'est voulu, le catalogue est bâti sur l'ACB seul (0,10 Gio à parcourir, pas 7,49 Gio).

Volumes mesurés ci-dessus : **ACB = 0,103 Gio, AWB = 7,688 Gio** (le rapport cité dans
`packages/azalee/src/cpk/audio.ts:10` — « 0,10 Gio d'ACB contre 7,49 Gio d'AWB » — est la même
mesure à l'échelle du jeu entier CPK+VFS ; celle d'ici porte le seul lot audio/vidéo).

**Nombre de cues réel, mesuré, pas estimé** — appel `/audio-info/<chemin>.acb` sur les 5 512 ACB
de l'inventaire (`nie-model-serve`, port 8790, 24 requêtes en parallèle, 7,5 s) puis
`jq -s '{total_cues: map(.cueCount)|add}'` :

```
banques ACB analysées : 5 512
cues totales           : 284 115
banques à 0 cue        : 0
banques non-ACB (bruit): 0
```

C'est ce nombre-là — **284 115**, pas 5 512 — qui doit apparaître dans une matrice/pagination de
catalogue audio. Exemple de granularité extrême déjà documenté : `waza_stream.acb` porte
1 512 cues à lui seul (`packages/azalee/src/cpk/audio.ts:4`) ; `bgm_title.acb` n'en porte qu'1
(cue `bg00010`, `cueId 10100`, 63,121 s, HCA 48 kHz stéréo, en boucle — vérifié en direct,
`/audio-info/data/common/sound_asset/bgm_title.acb`).

**Défaut déjà recensé** (`docs/PLAN-SITE-ULTIME.md` § « pièges d'édition » / § catalogage) : la
page « Sons » d'Aphrody catalogue aujourd'hui par fichier physique (`.awb` listé comme un son),
ce qui masque la structure réelle. Le bon niveau est le **cue**, adressé par `cueId` (l'identifiant
AFS2 stable), jamais par rang de fichier — cf. §7.

## 4. Ce que le dépôt sait déjà décoder

Module `crates/engine/nie-formats/src/cri_audio.rs` (972 lignes) :

- `adx_decode` (`cri_audio.rs:150`) — ADX → PCM16.
- `Awb::parse` / `parse_entete` (`cri_audio.rs:299,323,340`) — AFS2, table de cue-ids.
- `acb_parse` (`cri_audio.rs:486`) — `AcbInfo` (nom, `cue_count`, `cue_names`) depuis les `@UTF`
  imbriquées (CueTable, CueNameTable, WaveformTable).
- `acb_cues` (`cri_audio.rs:641`) — résolution complète cue → nom, durée, codec, fréquence,
  canaux, `awbId` — **sans ouvrir l'AWB**.
- `acb_stream_awb_header` (`cri_audio.rs:572`) — retrouve l'AWB frère depuis l'en-tête recopié
  dans l'ACB.
- HCA : `hca_decode_to_pcm16` (référencé `crates/tools/nie-model-serve/src/main.rs:6675`), chiffré
  (clé `subkey` par AWB), pur Rust.
- `decode_to_wav` (source unique, `cri_audio.rs`, wrappée par model-serve
  `main.rs:2385`) — HCA + ADX/AWB/ACB → WAV.

Module `crates/engine/nie-formats/src/usm.rs` (1 132 lignes) :

- Deux codecs vidéo distingués **par le champ `mpeg_codec` de `VIDEO_HDRINFO`**, jamais par
  reniflage d'octets (`usm.rs:64`) : `H264` (`mpeg_codec=5`, 95 des 97 films — navigateur-ready,
  `usm.rs:105,119`) et `Mpeg2` (`mpeg_codec=1`, les deux `.usm` loose `IE_15th`/`L5logo`, 2640×1080
  30 i/s — **aucun conteneur web pour ce codec**, `usm.rs:349` sort une erreur explicite plutôt
  qu'un flux corrompu).
- `ConteneurWeb` (`usm.rs:225`) — remux MP4/WebM du flux H.264 démuxé.
- Pistes audio détectées par les octets (`CodecAudio`, `usm.rs:137`) — 95 films sur 97 sont
  **muets** dans leur conteneur USM (le commentaire de `main.rs:5654` le documente) : leur son vit
  dans `anime_stream`, résolu par nom.

**Feature-gating à respecter** (piège documenté CLAUDE.md) : `nie-formats` n'active par défaut que
`std, lua`. `audio-decode` (→ `cridecoder`), `textures`, `images` sont **optionnelles, off par
défaut** — un `cargo test -p nie-formats` sans `--features audio-decode` ne teste RIEN de ce
domaine et rend un faux vert. `nie-model-serve` les active explicitement
(`crates/tools/nie-model-serve/Cargo.toml:26` :
`features = ["serde", "textures", "images", "audio-decode"]`) — c'est le binaire réellement
compilé qui sert ce domaine, à citer, pas la feature par défaut du crate.

`.p3lip` (lip-sync) : `nie_formats::lip::parse`, consommé par la route `/lip/` (§5) — décode en
visèmes datés (`{duration_s, frames:[{time_s, viseme, channel, param}]}`).

## 5. Ce que le service local sert déjà (mesuré, pas supposé)

`nie-site` (port 8085, Axum, `/api/v1/*`) proxifie `nie-model-serve` (port 8790, le vrai décodeur).
Les routes audio/vidéo vivent sur **8790**, pas sur `/assets/*` de 8085 comme le laisserait
supposer une convention générique — vérifié : `curl 127.0.0.1:8085/audio-info/...` rend du HTML
(la coquille Aphrody), pas du JSON ; c'est `127.0.0.1:8790/audio-info/...` qui répond.

Routes trouvées par balayage de `crates/tools/nie-model-serve/src/main.rs`
(`rg 'strip_prefix\("/'`) et testées par `curl` :

| Route | Test | HTTP | Taille | Notes |
|---|---|---:|---:|---|
| `GET /audio-info/<chemin>.acb` | `bgm_chronicle.acb` | **200** | 435 705 o | JSON, tous les cues |
| `GET /audio-info/<chemin>.acb` | `bgm_title.acb` | **200** | 176 o | 1 cue, `cueId 10100` |
| `GET /audio-info/<chemin>.awb` (direct, sans ACB) | `bgm_title.awb` | **200** | — | `cueCount:0`, `container:"acb"` mais champs `null` — **route mal ciblée, pas une absence de son** : l'AWB seul ne porte aucun métadonnée, il faut l'ACB frère |
| `GET /audio-info/<chemin>.usm` | `Chronicle_Title_EN_01.usm` | **200** | 182 o | JSON, mais décrit une absence : magic non-audio |
| `GET /audio/<chemin>.acb` | `bgm_title.acb` (décodage complet, cue par défaut) | **200** | 12 122 744 o | WAV décodé, cue par défaut = premier |
| `GET /audio/<chemin>.acb?id=<cueId>` | `bgm_title.acb?id=1` | **404** | 28 o | `cue-id 1 absent de la banque` — la banque n'a que `cueId=10100` : preuve que `?id=` prend le **cue-id AFS2**, pas un rang 0-based |
| `GET /audio/<chemin>.usm` | `Chronicle_Title_EN_01.usm` | **500** | 122 o | `format audio non reconnu (magic: CRID)` — attendu : `/audio/` ne sait pas lire un conteneur vidéo, il faut `/video/` |
| `GET /video/<chemin>.usm` | `dx11/movie/IE_15th.usm` | **200** | 4 170 113 o | MP4 remuxé — servi tel quel |
| `GET /video/<chemin>.usm?info=1` | `dx11/movie/IE_15th.usm` | **200** | JSON | `codec:"mpeg2"`, `lisibleNavigateur:false`, `remuxImpossible:"…ce codec n'a pas de conteneur web"` — MPEG-2 est décodé (métadonnées exactes) mais **non remuxable pour un `<video>` HTML** |
| `GET /export/<chemin>.usm` | `dx11/movie/IE_15th.usm` | **200** | 4 339 040 o | octets bruts du VFS, sans décodage |
| `GET /lip/<chemin>.p3lip` (ou `.json`) | `sound/en/ev01_00300_010_010.p3lip` | **200** | 913 o | JSON de visèmes |
| `GET /vfs/*` | `?path=` en query — testé avec le chemin **en segment** par erreur | **404** | 11 o | confirme le piège documenté : `/vfs/*` prend `?path=` en QUERY, jamais en segment |
| `GET /formats` | — | **404** | 11 o | n'existe pas sur 8790 ; c'est `/api/v1/formats` sur **8085** (nie-site) qui répond, **200**, 1 258 o |

Échantillon `.usm` (194 fichiers) via `/video/?info=1` — **balayage partiel, arrêté par un
timeout d'outil, pas par le service** : sur 16 réponses obtenues avant coupure (les 178 autres
sont des connexions dont l'issue n'est pas connue — **indéterminées, comptées à part, pas comme
échec**, conformément à la règle « code 0 ≠ échec ») : 9 `lisibleNavigateur:true` (H.264, remux
MP4 direct), 7 `remuxImpossible` (MPEG-2, les deux loose + doublons potentiels dans l'échantillon),
2 avec piste audio embarquée. **À refaire en balayage complet avec un timeout par requête et un
budget total dimensionné (le remux MPEG-2 échoue vite ; le remux H.264 des 97 films peut prendre
plusieurs dizaines de secondes chacun — le commentaire du code lui-même dit qu'un calcul à la
volée « tiendrait la connexion une minute »).**

## 6. Tableau de couverture (matrice `docs/PLAN-SITE-ULTIME.md` § 4)

| Capacité | État | Preuve / raison |
|---|---|---|
| Catalogue des cues d'une banque ACB | **servi** | `/audio-info/<x>.acb` → 200, JSON complet (`AcbInfo`/`AcbCue`) |
| Décodage WAV d'une cue par cue-id | **servi** | `/audio/<x>.acb?id=<cueId>` → 200 (testé avec un id valide implicitement via le défaut ; id explicite validé par le 404 correctement typé) |
| Décodage WAV du premier flux d'une banque (sans id) | **servi** | `/audio/<x>.acb` → 200, 12,1 Mo WAV |
| Lecture d'un AWB seul (sans ACB) | **interne, raison mesurée** | route répond 200 mais rend un JSON vide de sens (`cueCount:0`) — l'AWB seul n'a pas de métadonnée exploitable ; **pas un bug**, un usage qui n'a pas de forme correcte (toujours passer par l'ACB) |
| Remux H.264 → MP4 | **servi** | `/video/<x>.usm` → 200 sur l'échantillon H.264 |
| Remux MPEG-2 → conteneur web | **manquant** | `remuxImpossible` explicite, cause connue (pas de conteneur web pour Sofdec Prime) ; concerne 2 fichiers loose (`IE_15th`, `L5logo`) sur 97 films logiques, donc marginal en volume mais total en visibilité (ce sont les vidéos de lancement) |
| Bande-son croisée USM ↔ `anime_stream` | **interne** | `nie_explore::cinema` résout par nom (commentaire `main.rs:5654`), pas exposé en route dédiée testée ici — à vérifier séparément |
| Décodage lip-sync `.p3lip` | **servi** | `/lip/<x>.p3lip` → 200, visèmes datés |
| Export brut (octets VFS, sans décodage) | **servi** | `/export/<x>` → 200 pour n'importe quelle extension du domaine |
| Total de cues à l'échelle du domaine | **interne (mesuré ici, non exposé en API)** | 284 115, calculé par agrégation côté client (`jq -s`) sur 5 512 appels `/audio-info` — pas d'endpoint qui le rend en un appel |
| Catalogue "Sons" par cue (pas par fichier `.acb`/`.awb`) | **manquant côté front, servi côté API** | l'API (`/audio-info`) porte déjà la structure correcte ; c'est la page Sons d'Aphrody qui affiche l'AWB comme une piste — défaut recensé au plan |
| Page "Vidéos" listant les 97 films avec statut de lecture | **à vérifier** | l'API répond par fichier (`/video/?info=1`) ; pas constaté ici si une page liste les 97 avec leur `lisibleNavigateur` |
| Téléchargement nommé par cue (pas par fichier source) | **manquant** | aucune route observée ne pose `Content-Disposition` avec un nom dérivé du `cueId`/nom de cue — cf. §7, défaut de la même famille que celui déjà payé sur les textures nommées d'un G4TX |

## 7. Routes à créer

1. **`GET /audio-download/<chemin>.acb?id=<cueId>`** — variante de `/audio/` avec
   `Content-Disposition: attachment; filename="<nomCue>-<cueId>.wav"`. **Nommer par le cue, jamais
   par le fichier `.acb` source** : sans ça, tous les téléchargements d'un même `bgm.acb` (des
   centaines de cues) se recouvrent sous `bgm.wav`. Résoudre le nom par `acb_cues[].name`
   (`cri_audio.rs:598`), avec repli sur `cue-<cueId>` si le nom est vide (les cues sans nom
   existent, `cri_audio.rs:639`). Parseur déjà là : `acb_cues` + `decode_awb_entry`
   (`main.rs:2395`).
2. **`GET /api/v1/audio/cues-total`** (ou champ ajouté à `/api/v1/formats`) — expose 284 115 (ou
   la valeur rejouée) sans recalcul côté client à chaque visite. Calcul en tâche de fond
   (`nie-forge`-like), pas synchrone : 5 512 lectures ACB à chaud coûtent ~7 s, trop pour une
   requête de page.
3. **`GET /video/<chemin>.usm?piste=voix`** ou équivalent qui **résout la bande-son croisée**
   (`anime_stream`) au lieu de renvoyer une vidéo muette pour les 95 films sans piste native —
   la logique de résolution par nom existe déjà (`nie_explore::cinema`, cité en commentaire), reste
   à la brancher en route si ce n'est pas déjà le cas côté `/video/?track=audio` (à vérifier, le
   fichier continue après la ligne lue ici, `main.rs:5680+`).
4. **Page catalogue "Sons" réécrite par cue** — chaque ligne = un `AudioCue` (`cueId`, `name`,
   `durationSec`, `codec`), groupé par banque ACB pour le contexte, jamais listé au niveau AWB.
   Données déjà disponibles via `@niers/catalog/jeu` (`AudioBank`/`AudioCue`,
   `packages/nie-catalog/src/jeu.ts:601-648`) et `packages/azalee/src/cpk/audio.ts`
   (`cpkAudioCueUrl`, adressage par `awbId`/cue-id, pas par rang — déjà correct côté azalee, à
   répliquer côté Aphrody).
5. **Page "Vidéos" avec statut par film** — 97 films logiques (`common`+`dx11`, jumeaux),
   `lisibleNavigateur` en badge, fallback MPEG-2 = lien `/export/` brut + mention explicite
   "non lisible dans le navigateur, codec Sofdec Prime".
6. **`GET /lip/<chemin>.p3lip` déjà exploitable tel quel** en overlay de sous-titres/visèmes sur
   une lecture audio de dialogue — pas de route à créer, juste à câbler côté front avec le lecteur
   audio du cue correspondant (le radical du `.p3lip` est celui de l'événement, pas un cue-id —
   jointure à faire par nom, cf. §8).

## 8. Ce que le mode de jeu attend (avec ses trous connus)

- **BGM** (`bgm.acb`, `bgm_chronicle.acb`, `bgm_title.acb`) : musique de fond par écran/match —
  `bgm_config_0.00.00.cfg.bin` (`data/common/sound/`) porte vraisemblablement le mapping
  écran → cue, **non vérifié ici** (hors périmètre audio pur, format config du domaine 1).
- **Voix** (`sound_asset/<langue>/partvoice_sub2.acb`, `anime_stream_voice.awb`) : dialogues
  localisés, croisés avec les `.p3lip` du même radical événementiel pour la synchro labiale — la
  jointure exacte event↔cue↔lip **n'est pas prouvée ici**, à vérifier via `inagle_event_subtitles`
  (mentionné dans la mémoire du dépôt pour le mode histoire) avant de l'affirmer.
- **SE / commentateur** (`caster_set_*_commentary_trigger`, `*_sequence_se_config`) : déclencheurs
  d'effets sonores et de commentaire de match — format config, hors décodage audio propre, à
  documenter dans le domaine "gamedata/config" plutôt qu'ici.
- **Cinématiques** (`waza_stream.acb`, `.usm` du dossier `movie/`) : `waza_stream` (1 512 cues,
  291,5 Mio) sert vraisemblablement les mises en scène de tir spécial ("Hissatsu"/tactiques) et
  les intros de match, sur la base du nom seul — **à confirmer**, aucun croisement `inagle_*`
  vérifié dans cette passe.
- **Jumeaux `common/movie` vs `dx11/movie`** : mêmes 97 noms, tailles différentes — probablement
  deux profils d'encodage (bas/haut débit) sélectionnés selon la plateforme ou la qualité vidéo
  réglée en jeu ; **non confirmé par du code lu ici**, à vérifier avant d'écrire une route qui
  choisirait l'un plutôt que l'autre par défaut.

## 9. Commandes de vérification (rejouables telles quelles)

```bash
# Décompte par extension
awk '{print $1}' var/vfs/lot4-audio.txt | sed -E 's/.*(\.[a-zA-Z0-9]+)$/\1/' | sort | uniq -c | sort -rn

# Poids par extension
for ext in p3lip awb acb usm bin acf; do
  awk -v e=".$ext" '$1 ~ e"$"{s+=$2; n++} END{printf "%-8s n=%-7d octets=%-14d Gio=%.3f\n", e, n, s, s/1024/1024/1024}' var/vfs/lot4-audio.txt
done

# Total de cues (7-10 s, exige nie-model-serve actif sur :8790)
awk '$1 ~ /\.acb$/{print $1}' var/vfs/lot4-audio.txt > /tmp/acb_list.txt
mkdir -p /tmp/acb_out
awk '{print NR" "$0}' /tmp/acb_list.txt | xargs -P 24 -L1 bash -c \
  'curl -s "http://127.0.0.1:8790/audio-info/$1" -o "/tmp/acb_out/$0.json"'
jq -s '{total_cues: (map(.cueCount // 0) | add), banques: length}' /tmp/acb_out/*.json
```
