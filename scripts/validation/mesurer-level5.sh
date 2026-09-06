#!/usr/bin/env bash
# Mesure le decodage des CINQ familles cablees le 2026-09-06 apres la matrice de couverture :
# `.p3lip`, `.g4nv`, `.g4ma`, `.g4vs`, `.g4la` — 21 250 fichiers dont le parseur existait dans
# `nie-formats` sans qu'aucune route ne l'appelle.
#
#   scripts/validation/mesurer-level5.sh [base] [par_famille]
#
#   base         URL du service            (defaut http://127.0.0.1:8085)
#   par_famille  echantillon par extension (defaut 40)
#
# La mesure exige le JETON de la famille dans le corps, pas un code 200 : `/api/v1/formats`
# repond 200 en disant « format non identifie », et compter ce 200 pour une couverture serait
# exactement le faux vert que ce depot a deja paye.
#
# L'echantillon est pris a PAS REGULIER sur la liste triee, jamais au hasard ni en tete : les
# premiers fichiers d'un dossier se ressemblent, et un echantillon de tete mesure un dossier,
# pas un corpus.
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"
PAR_FAMILLE="${2:-40}"
RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INVENTAIRE="$RACINE/var/vfs/inventaire.txt"

[ -f "$INVENTAIRE" ] || { echo "inventaire absent: $INVENTAIRE" >&2; exit 1; }

# Le chemin se lit en retirant les DEUX derniers champs (taille, cpk) : des chemins du VFS
# contiennent un espace, et un decoupage par espaces croissants les casse.
CHEMINS=$(mktemp); trap 'rm -f "$CHEMINS"' EXIT
sed -E 's/ [0-9]+ \[[^]]*\]$//; s/ [0-9]+ \[?[^ ]*\]?$//' "$INVENTAIRE" > "$CHEMINS"

total=0; ok=0
printf '%-8s %7s %7s  %s\n' famille testes reussis exemple

for ext in p3lip g4nv g4ma g4vs g4la; do
    liste=$(grep "\.$ext\$" "$CHEMINS" || true)
    n=$(printf '%s\n' "$liste" | grep -c . || true)
    [ "$n" -gt 0 ] || { printf '%-8s %7s %7s  %s\n' "$ext" 0 0 "aucun fichier"; continue; }
    pas=$(( n / PAR_FAMILLE )); [ "$pas" -ge 1 ] || pas=1

    f_total=0; f_ok=0; exemple=""
    while IFS= read -r chemin; do
        [ -n "$chemin" ] || continue
        f_total=$(( f_total + 1 ))
        corps=$(curl -s --max-time 60 \
            "$BASE/api/v1/formats/decode/$(printf '%s' "$chemin" | sed 's/ /%20/g')" || true)
        if printf '%s' "$corps" | jq -e --arg f "$ext" '.format == $f' >/dev/null 2>&1; then
            f_ok=$(( f_ok + 1 ))
            [ -n "$exemple" ] || exemple=$(printf '%s' "$corps" | jq -c '.resume' | head -c 90)
        else
            echo "  ECHEC $chemin -> $(printf '%s' "$corps" | head -c 120)" >&2
        fi
    done < <(printf '%s\n' "$liste" | awk -v p="$pas" 'NR % p == 1 || p == 1')

    printf '%-8s %7s %7s  %s\n' "$ext" "$f_total" "$f_ok" "$exemple"
    total=$(( total + f_total )); ok=$(( ok + f_ok ))
done

echo
printf 'total: %d/%d decodes (%d%%)\n' "$ok" "$total" $(( total > 0 ? ok * 100 / total : 0 ))
[ "$ok" -eq "$total" ]
