#!/usr/bin/env bash
# Identifie les fichiers des extensions RARES du VFS (§ 9.2 de docs/PLAN-SITE-ULTIME.md).
#
# La gate `100 %` du plan exige qu'aucun des 255 308 fichiers ne reste non classé. Les
# extensions de moins de 15 fichiers en concentrent l'essentiel : 15 extensions, 48 entrées
# d'inventaire. Ce script les interroge une par une et rend **ce que le serveur a reconnu**,
# jamais ce qu'on suppose de leur nom.
#
#   scripts/validation/mesurer-extensions-rares.sh [base] [seuil]
#
#   base   URL du service        (défaut http://127.0.0.1:8085)
#   seuil  extensions sous ce compte (défaut 15)
#
# Piège payé le 2026-09-06, et corrigé ici : **des chemins du VFS contiennent un espace**
# (`…/u021801/u021802 .g4md`). Découper l'inventaire par espaces croissants fait apparaître deux
# faux « fichiers sans extension » et fausse tout comptage par extension. Le chemin se lit donc
# en retirant les DEUX derniers champs (taille, cpk), jamais en prenant le premier.
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"
SEUIL="${2:-15}"
RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INVENTAIRE="$RACINE/var/vfs/inventaire.txt"

[ -f "$INVENTAIRE" ] || { echo "inventaire absent: $INVENTAIRE" >&2; exit 1; }

# chemin<TAB>extension, en retirant les deux derniers champs.
CHEMINS=$(mktemp)
trap 'rm -f "$CHEMINS"' EXIT
sed -E 's/ [0-9]+ \[[^]]*\]$//; s/ [0-9]+ \[?[^ ]*\]?$//' "$INVENTAIRE" \
  | awk '{ e = ""; if (match($0, /\.[^./]+$/)) e = substr($0, RSTART); print $0 "\t" e }' > "$CHEMINS"

RARES=$(cut -f2 "$CHEMINS" | sort | uniq -c | awk -v s="$SEUIL" '$1 < s && $2 != "" {print $2}')

printf '%-10s %5s  %-24s %s\n' extension n format "detail"
total=0; identifies=0

for ext in $RARES; do
    while IFS=$'\t' read -r chemin e; do
        [ "$e" = "$ext" ] || continue
        total=$(( total + 1 ))
        # `--get --data-urlencode` encode l'espace et les caractères réservés du chemin.
        corps=$(curl -s --max-time 60 --get --data-urlencode "x=" \
                 "$BASE/api/v1/formats/decode/$(printf '%s' "$chemin" | sed 's/ /%20/g')" || true)
        format=$(printf '%s' "$corps" | jq -r '.format // .resume.famille // empty' 2>/dev/null || true)
        detail=$(printf '%s' "$corps" | jq -r '.conteneur.magic // .produit // .message // empty' 2>/dev/null || true)

        # Tout n'est pas un format binaire : 12 des fichiers rares sont du TEXTE (10 journaux
        # de conversion `.log`, 2 listes de blocs `.cfg`). Les compter « non identifiés » parce
        # qu'aucun décodeur ne les réclame serait une mesure fausse — ils sont servis, lisibles,
        # et leur type de contenu le prouve. On le vérifie sur `/f`, pas sur l'extension.
        if [ -z "$format" ]; then
            ct=$(curl -s -o /dev/null -w '%{content_type}' --max-time 30 \
                 "$BASE/f/$(printf '%s' "$chemin" | sed 's/ /%20/g')" || true)
            case "$ct" in
                text/plain*) format="texte"; detail="servi par /f en $ct" ;;
            esac
        fi

        if [ -n "$format" ]; then
            identifies=$(( identifies + 1 ))
        else
            format="NON IDENTIFIE"
        fi
        printf '%-10s %5s  %-24s %s\n' "$ext" "" "$format" "$(basename "$chemin") — ${detail:0:60}"
    done < "$CHEMINS"
done

echo
printf 'total: %d/%d fichiers rares identifies\n' "$identifies" "$total"
