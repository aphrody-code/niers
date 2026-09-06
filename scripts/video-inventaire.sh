#!/usr/bin/env bash
# Inventaire complet et rejouable des cinematiques .usm d'Inazuma Eleven: Victory Road.
#
# Pour chacune des entrees .usm du VFS (les deux montages `data/common/movie` et
# `data/dx11/movie`), le script mesure :
#   * ce que declare le CONTENEUR USM       -> `niers video liste --rapide` (tables @UTF)
#   * ce que mesure le FLUX ELEMENTAIRE     -> `ffprobe` sur l'export produit par `niers video export`
#   * la presence au CATALOGUE servi         -> jointure sur var/model-cache/video-catalog.json
#
# Rien n'est suppose : le codec, la definition et le nombre d'images lus par ffprobe viennent
# du flux reellement decode, pas des en-tetes du conteneur. Les deux sont conserves cote a cote
# pour que toute divergence soit visible (elle l'est : voir le champ `divergences`).
#
# Sortie : var/video-audit/inventaire-usm.json  (+ un resume sur la sortie standard)
#
# Usage :
#   scripts/video-inventaire.sh                       # les 194 entrees, ffprobe compris
#   scripts/video-inventaire.sh --sans-ffprobe        # en-tetes seuls, ~4 min
#   scripts/video-inventaire.sh --montage common      # un seul montage
#   scripts/video-inventaire.sh --max-octets 400000000  # saute les films plus gros (RAM)
#   scripts/video-inventaire.sh --limite 5            # echantillon
#
# Cout mesure : ~20 Go lus depuis les CPK (3,7 Go common + 15,8 Go dx11). Les exports sont
# ecrits un par un dans var/video-audit/tmp puis EFFACES : le pic disque vaut le plus gros
# film exporte, jamais la somme.

set -euo pipefail

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SORTIE_DIR="$RACINE/var/video-audit"
TMP="$SORTIE_DIR/tmp"
CATALOGUE="$RACINE/var/model-cache/video-catalog.json"
NIERS="${NIERS:-niers}"

MONTAGE=tous
LIMITE=0
MAX_OCTETS=0
FFPROBE=1
RAFRAICHIR=1
REPRENDRE=0

while [ $# -gt 0 ]; do
  case "$1" in
    --montage)      MONTAGE="$2"; shift 2 ;;
    --limite)       LIMITE="$2"; shift 2 ;;
    --max-octets)   MAX_OCTETS="$2"; shift 2 ;;
    --sans-ffprobe) FFPROBE=0; shift ;;
    --reutiliser)   RAFRAICHIR=0; shift ;;   # reutilise les listes @UTF deja produites
    --reprendre)    REPRENDRE=1; shift ;;    # garde les films deja sondes (reprise apres coupure)
    -h|--help)      sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "option inconnue : $1" >&2; exit 2 ;;
  esac
done

command -v "$NIERS"  >/dev/null || { echo "niers introuvable"  >&2; exit 1; }
command -v ffprobe   >/dev/null || { echo "ffprobe introuvable" >&2; exit 1; }
command -v jq        >/dev/null || { echo "jq introuvable"      >&2; exit 1; }

mkdir -p "$TMP"
export NIE_GAME_DIR="${NIE_GAME_DIR:-$RACINE}"
cd "$RACINE"

# ── 1. L'index VFS : la seule source du compte d'entrees .usm ────────────────────
echo "[1/4] index VFS des .usm" >&2
INDEX="$SORTIE_DIR/index-usm.json"
if [ "$RAFRAICHIR" = 1 ] || [ ! -s "$INDEX" ]; then
  "$NIERS" vfs find .usm --ext usm -n 1000 -j > "$INDEX"
fi
echo "      $(jq 'length' "$INDEX") entrees" >&2

# ── 2. Les en-tetes @UTF des deux montages ───────────────────────────────────────
echo "[2/4] metadonnees conteneur (tables @UTF)" >&2
for m in common dx11; do
  case "$MONTAGE" in tous) ;; "$m") ;; *) continue ;; esac
  f="$SORTIE_DIR/liste-$m.json"
  if [ "$RAFRAICHIR" = 1 ] || [ ! -s "$f" ]; then
    "$NIERS" video liste --prefixe "data/$m/movie" --json --rapide > "$f"
  fi
  echo "      $m : $(jq '.films|length' "$f") films" >&2
done

# ── 3. ffprobe, film par film, sur l'export reel ─────────────────────────────────
LIGNES="$TMP/lignes.ndjson"
[ "$REPRENDRE" = 1 ] || : > "$LIGNES"
touch "$LIGNES"

mapfile -t CHEMINS < <(jq -r --arg m "$MONTAGE" \
  '.[] | select($m == "tous" or (.path | split("/")[1]) == $m) | .path' "$INDEX" | sort)

[ "$LIMITE" -gt 0 ] && CHEMINS=("${CHEMINS[@]:0:$LIMITE}")
TOTAL=${#CHEMINS[@]}
echo "[3/4] ffprobe sur $TOTAL film(s) (ffprobe=$FFPROBE)" >&2

i=0
for chemin in "${CHEMINS[@]}"; do
  i=$((i + 1))
  # Reprise : un film deja sonde (ffprobe non nul) n'est pas relu depuis le CPK.
  if [ "$REPRENDRE" = 1 ] && [ -s "$LIGNES" ] \
     && jq -e --arg p "$chemin" 'select(.chemin==$p and .ffprobe != null)' "$LIGNES" >/dev/null 2>&1; then
    printf '\r      %3d/%d  %-34s (deja sonde)' "$i" "$TOTAL" "$(basename "$chemin" .usm)" >&2
    continue
  fi
  nom="$(basename "$chemin" .usm)"
  montage="$(echo "$chemin" | cut -d/ -f2)"
  octets="$(jq -r --arg p "$chemin" '.[]|select(.path==$p)|.size' "$INDEX")"
  probe='null'; probe_audio='null'; note='null'

  if [ "$FFPROBE" = 1 ] && { [ "$MAX_OCTETS" = 0 ] || [ "$octets" -le "$MAX_OCTETS" ]; }; then
    rm -f "$TMP/piste."*
    base="$TMP/piste"
    # Remux si un conteneur web existe pour ce codec, flux elementaire sinon.
    if ! "$NIERS" video export "$chemin" --out "$base.mp4" --audio >"$TMP/export.log" 2>&1; then
      "$NIERS" video export "$chemin" --out "$base.mp4" --brut --audio >"$TMP/export.log" 2>&1 || true
    fi
    piste="$(ls "$TMP"/piste.mp4 "$TMP"/piste.webm "$TMP"/piste.m2v "$TMP"/piste.h264 \
              "$TMP"/piste.ivf "$TMP"/piste.bin 2>/dev/null | head -1 || true)"
    if [ -n "$piste" ] && [ -s "$piste" ]; then
      # `-count_frames` DECODE chaque image : plusieurs minutes sur un film 1080p de 9 000
      # images. Un MP4/WebM porte deja `nb_frames` dans son conteneur (mesure : 0,099 s au lieu
      # de plusieurs minutes) — le comptage n'est donc paye que pour un flux elementaire, ou
      # rien ne declare le nombre d'images.
      case "${piste##*.}" in
        mp4|webm) COMPTE=() ;;
        *)        COMPTE=(-count_frames) ;;
      esac
      probe="$(ffprobe -v error "${COMPTE[@]}" -probesize 200M -analyzeduration 200M \
                 -show_streams -show_format -of json "$piste" 2>/dev/null \
               | jq -c --arg ext "${piste##*.}" --arg o "$(stat -c%s "$piste")" '
                   (.streams[]? | select(.codec_type=="video")) as $v
                   | {conteneurExport:$ext, octetsExport:($o|tonumber),
                      format:(.format.format_name//null),
                      codec:($v.codec_name//null), profil:($v.profile//null),
                      largeur:($v.width//null), hauteur:($v.height//null),
                      pixFmt:($v.pix_fmt//null),
                      imagesLues:(($v.nb_read_frames // $v.nb_frames // "0")|tonumber),
                      cadence:($v.avg_frame_rate//null),
                      duree:(($v.duration//.format.duration)|if .==null then null else tonumber end),
                      debit:(($v.bit_rate//.format.bit_rate)|if .==null then null else tonumber end)}' \
               || echo null)"
      [ -z "$probe" ] && probe=null
    else
      note="\"export impossible : $(tr -d '\n\"' < "$TMP/export.log" | tail -c 200)\""
    fi
    wav="$(ls "$TMP"/piste.*.wav "$TMP"/piste.wav 2>/dev/null | head -1 || true)"
    if [ -n "$wav" ] && [ -s "$wav" ]; then
      probe_audio="$(ffprobe -v error -show_streams -show_format -of json "$wav" 2>/dev/null \
        | jq -c '(.streams[0]//{}) | {codec:(.codec_name//null), frequence:((.sample_rate//"0")|tonumber),
                  canaux:(.channels//null), duree:((.duration//"0")|tonumber)}' || echo null)"
      [ -z "$probe_audio" ] && probe_audio=null
    fi
    rm -f "$TMP"/piste.* "$TMP/export.log"
  elif [ "$FFPROBE" = 1 ]; then
    note="\"non sonde : $octets octets > --max-octets $MAX_OCTETS\""
  fi

  printf '{"chemin":%s,"montage":%s,"nom":%s,"octets":%s,"ffprobe":%s,"ffprobeAudio":%s,"note":%s}\n' \
    "$(jq -Rn --arg v "$chemin" '$v')" "$(jq -Rn --arg v "$montage" '$v')" \
    "$(jq -Rn --arg v "$nom" '$v')" "$octets" "$probe" "$probe_audio" "$note" >> "$LIGNES"

  printf '\r      %3d/%d  %-34s' "$i" "$TOTAL" "$nom" >&2
done
echo >&2

# ── 4. Jointure : index VFS + @UTF + ffprobe + catalogue ─────────────────────────
echo "[4/4] jointure et ecriture" >&2
CAT_ARG="$CATALOGUE"; [ -s "$CAT_ARG" ] || CAT_ARG=/dev/null

jq -n \
  --slurpfile index "$INDEX" \
  --slurpfile cat   <(if [ -s "$CATALOGUE" ]; then cat "$CATALOGUE"; else echo '{"films":[]}'; fi) \
  --argjson lignes "$(jq -s 'group_by(.chemin) | map(.[-1]) | sort_by(.chemin)' "$LIGNES")" \
  --arg genere "$(date -Is)" \
  --arg utf_common "$SORTIE_DIR/liste-common.json" \
  --arg utf_dx11   "$SORTIE_DIR/liste-dx11.json" \
  --slurpfile uc "$(if [ -s "$SORTIE_DIR/liste-common.json" ]; then echo "$SORTIE_DIR/liste-common.json"; else echo /dev/null; fi)" \
  --slurpfile ud "$(if [ -s "$SORTIE_DIR/liste-dx11.json" ];   then echo "$SORTIE_DIR/liste-dx11.json";   else echo /dev/null; fi)" '
  ($index[0] // []) as $idx
  | (($uc[0].films // []) + ($ud[0].films // [])) as $utf
  | ($utf | map({key: .chemin, value: .}) | from_entries) as $UTF
  | (($cat[0].films // []) | map({key: .chemin, value: .}) | from_entries) as $CAT
  | ($idx | map({key: .path, value: .}) | from_entries) as $IDX
  | {
      genere: $genere,
      source: {index: "niers vfs find --ext usm", conteneur: "niers video liste --rapide",
               flux: "ffprobe -count_frames sur niers video export", catalogue: "var/model-cache/video-catalog.json"},
      entrees: ($lignes | length),
      films: [ $lignes[] | . as $l
        | ($UTF[$l.chemin] // null) as $u
        | {
            chemin: $l.chemin, nom: $l.nom, montage: $l.montage,
            octets: $l.octets, cpk: ($IDX[$l.chemin].cpk // null),
            conteneur: (if $u == null then null else {
              codec: $u.codec, largeur: $u.largeur, hauteur: $u.hauteur,
              images: $u.images, cadence: $u.cadence, duree: $u.duree,
              rubrique: $u.rubrique, langue: $u.langue,
              lisibleNavigateur: $u.lisibleNavigateur,
              audio: ($u.audio // []), nomOrigine: $u.nomOrigine,
              erreur: ($u.erreur // null) } end),
            ffprobe: $l.ffprobe, ffprobeAudio: $l.ffprobeAudio,
            auCatalogue: ($CAT[$l.chemin] != null),
            note: $l.note
          }
        | . + { divergences: [
            (if (.conteneur != null and .ffprobe != null and .ffprobe.largeur != null
                 and .conteneur.largeur != .ffprobe.largeur) then "largeur" else empty end),
            (if (.conteneur != null and .ffprobe != null and .ffprobe.hauteur != null
                 and .conteneur.hauteur != .ffprobe.hauteur) then "hauteur" else empty end),
            (if (.conteneur != null and .ffprobe != null and .ffprobe.imagesLues != null
                 and .ffprobe.imagesLues > 0 and .conteneur.images != .ffprobe.imagesLues)
             then "images" else empty end),
            (if (.conteneur != null and .ffprobe != null and .ffprobe.codec != null
                 and ((.conteneur.codec // "") | ascii_downcase) != ((.ffprobe.codec // "") | ascii_downcase))
             then "codec" else empty end)
          ] }
      ]
    }
  | . + { resume: {
      parMontage: (.films | group_by(.montage) | map({(.[0].montage): length}) | add),
      auCatalogue: (.films | map(select(.auCatalogue)) | length),
      horsCatalogue: (.films | map(select(.auCatalogue | not)) | length),
      codecConteneur: (.films | map(.conteneur.codec // "inconnu") | group_by(.) | map({(.[0]): length}) | add),
      codecFfprobe: (.films | map(.ffprobe.codec // "non sonde") | group_by(.) | map({(.[0]): length}) | add),
      avecAudioConteneur: (.films | map(select((.conteneur.audio // []) | length > 0)) | length),
      octets: (.films | map(.octets) | add)
    } }
  ' > "$SORTIE_DIR/inventaire-usm.json"

jq -r '.resume | "entrees        \(.parMontage)\nau catalogue   \(.auCatalogue)\nhors catalogue \(.horsCatalogue)\ncodec (USM)    \(.codecConteneur)\ncodec (ffprobe)\(.codecFfprobe)\navec audio     \(.avecAudioConteneur)"' \
  "$SORTIE_DIR/inventaire-usm.json"
echo "-> $SORTIE_DIR/inventaire-usm.json"

# ── 5. Fusion des passages + catalogue a variantes ───────────────────────────────
#
# Le montage `common` et le montage `dx11` sont sondes en deux passages (memoire : un film dx11
# de 2 Go alloue ~5 Go au demultiplexage). Cette etape recolle TOUS les NDJSON produits sous
# $SORTIE_DIR (`tmp*/lignes.ndjson`) en un seul inventaire des 194 entrees, puis derive le
# catalogue a variantes : 97 films, chacun avec ses DEUX fichiers.
echo "[5/5] fusion des passages" >&2
cat "$SORTIE_DIR"/tmp*/lignes.ndjson 2>/dev/null | LC_ALL=C sort -u > "$TMP/lignes-194.ndjson"

jq -n --slurpfile idx "$INDEX" \
      --slurpfile uc "$SORTIE_DIR/liste-common.json" --slurpfile ud "$SORTIE_DIR/liste-dx11.json" \
      --slurpfile cat "$CATALOGUE" --arg g "$(date -Is)" \
      --argjson l "$(jq -s 'group_by(.chemin)|map(.[-1])|sort_by(.chemin)' "$TMP/lignes-194.ndjson")" '
  ($idx[0]|map({key:.path,value:.})|from_entries) as $I
  | ((($uc[0].films)//[])+(($ud[0].films)//[])|map({key:.chemin,value:.})|from_entries) as $U
  | ((($cat[0].films)//[])|map({key:.chemin,value:.})|from_entries) as $C
  | {genere:$g, entrees:($l|length),
     films:[$l[]| . as $x | ($U[$x.chemin]) as $u | {
       chemin:$x.chemin, nom:$x.nom, montage:$x.montage, octets:$x.octets, cpk:($I[$x.chemin].cpk//null),
       conteneur:{codec:$u.codec, largeur:$u.largeur, hauteur:$u.hauteur, images:$u.images,
                  cadence:$u.cadence, duree:$u.duree, rubrique:$u.rubrique, langue:$u.langue,
                  lisibleNavigateur:$u.lisibleNavigateur, audio:($u.audio//[]),
                  bandeSon:($u.bandeSon//null), erreur:($u.erreur//null)},
       ffprobe:$x.ffprobe, ffprobeAudio:$x.ffprobeAudio,
       auCatalogue:($C[$x.chemin]!=null), note:$x.note}]}
  | . + {resume:{
      parMontage:(.films|group_by(.montage)|map({(.[0].montage):length})|add),
      auCatalogue:(.films|map(select(.auCatalogue))|length),
      horsCatalogue:(.films|map(select(.auCatalogue|not))|length),
      demuxEnErreur:(.films|map(select(.conteneur.erreur!=null))|length),
      codecUSM:(.films|group_by(.montage)|map({(.[0].montage):(map(.conteneur.codec)|group_by(.)|map({(.[0]):length})|add)})|add),
      codecFfprobe:(.films|map(.ffprobe.codec//"non sonde")|group_by(.)|map({(.[0]):length})|add),
      sondesFfprobe:(.films|group_by(.montage)|map({(.[0].montage):(map(select(.ffprobe!=null))|length)})|add),
      avecAudioInterne:(.films|map(select((.conteneur.audio|length)>0))|length),
      avecBandeSon:(.films|map(select(.conteneur.bandeSon!=null))|length),
      octets:(.films|map(.octets)|add)}}' > "$SORTIE_DIR/inventaire-194.json"

jq -s '
  (.[0].films) as $cat | (.[1].films|map({key:.nom,value:.})|from_entries) as $C
  | (.[2].films|map({key:.nom,value:.})|from_entries) as $D | (.[3]|map({key:.path,value:.})|from_entries) as $I
  | {genere:(now|todate),
     note:"97 FILMS, 194 FICHIERS. Chaque film existe en deux variantes portant le meme nom : data/common/movie (H.264/VP9/MPEG, la variante que declare le gamedata) et data/dx11/movie (MPEG, environ 3,8x plus lourde, meme duree a 0,05 s pres). Le catalogue servi expose uniquement la variante common.",
     films:($cat|map(. as $f | $f + {variantes:[
       {montage:"common", chemin:$f.chemin, octets:($I[$f.chemin].size//$f.octets), octetsLus:$C[$f.nom].octets,
        codec:$C[$f.nom].codec, largeur:$C[$f.nom].largeur, hauteur:$C[$f.nom].hauteur, images:$C[$f.nom].images,
        cadence:$C[$f.nom].cadence, duree:$C[$f.nom].duree, lisibleNavigateur:$C[$f.nom].lisibleNavigateur, cpk:($I[$f.chemin].cpk//null)},
       {montage:"dx11", chemin:("data/dx11/movie/"+$f.nom+".usm"), octets:($I["data/dx11/movie/"+$f.nom+".usm"].size//null),
        octetsLus:$D[$f.nom].octets, codec:$D[$f.nom].codec, largeur:$D[$f.nom].largeur, hauteur:$D[$f.nom].hauteur,
        images:$D[$f.nom].images, cadence:$D[$f.nom].cadence, duree:$D[$f.nom].duree,
        lisibleNavigateur:$D[$f.nom].lisibleNavigateur, cpk:($I["data/dx11/movie/"+$f.nom+".usm"].cpk//null)}]}
       | . + {variantesDivergentes:[
           (if $C[$f.nom].codec != $D[$f.nom].codec then "codec" else empty end),
           (if $C[$f.nom].largeur != $D[$f.nom].largeur or $C[$f.nom].hauteur != $D[$f.nom].hauteur then "definition" else empty end),
           (if $C[$f.nom].images != $D[$f.nom].images then "images" else empty end),
           (if ((($C[$f.nom].duree//0)-($D[$f.nom].duree//0))|fabs) > 0.05 then "duree" else empty end)]})}
  | . + {resume:{films:(.films|length), fichiers:((.films|length)*2),
      variantesIdentiques:(.films|map(select(.variantesDivergentes|length==0))|length),
      divergences:(.films|map(.variantesDivergentes)|flatten|group_by(.)|map({(.[0]):length})|add)}}
  ' "$CATALOGUE" "$SORTIE_DIR/liste-common.json" "$SORTIE_DIR/liste-dx11.json" "$INDEX" \
  > "$SORTIE_DIR/catalogue-variantes.json"

jq -r '.resume|"194 entrees : \(.parMontage)  |  au catalogue \(.auCatalogue), hors catalogue \(.horsCatalogue)\ndemux en erreur \(.demuxEnErreur)  |  sondes ffprobe \(.sondesFfprobe)\ncodec USM \(.codecUSM)\ncodec ffprobe \(.codecFfprobe)"' "$SORTIE_DIR/inventaire-194.json"
echo "-> $SORTIE_DIR/inventaire-194.json"
echo "-> $SORTIE_DIR/catalogue-variantes.json"
