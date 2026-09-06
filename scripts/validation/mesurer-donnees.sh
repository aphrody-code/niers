#!/usr/bin/env bash
# Mesure /api/v1/donnees — les familles de donnees NOMMEES du jeu (nie_data::typed).
#
#   scripts/validation/mesurer-donnees.sh [base] [echantillon] [paralleles]
#
# Ce que la mesure exige : le champ `famille` NON NUL dans le corps. Un 200 ne prouve rien, et
# un 404 « aucune famille nommee » est un resultat LEGITIME — les 71 101 `.cfg.bin` du jeu ne
# sont pas tous du gamedata typé (les evenements, les placements et les sons n'en sont pas).
# La mesure separe donc les deux et rend les DEUX comptes : couverture typee, et refus propres.
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"
ECH="${2:-400}"   # `--cles` a la place : sonde UNE fois chaque cle distincte (exhaustif)
PAR="${3:-16}"
RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INVENTAIRE="$RACINE/var/vfs/inventaire.txt"
[ -f "$INVENTAIRE" ] || { echo "inventaire absent: $INVENTAIRE" >&2; exit 1; }

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
sed -E 's/ [0-9]+ \[[^]]*\]$//; s/ [0-9]+ \[?[^ ]*\]?$//' "$INVENTAIRE" \
  | grep '\.cfg\.bin$' > "$TMP/tous"

# Mode exhaustif : une sonde par CLE DE FAMILLE distincte, pas par fichier. C'est la seule
# mesure qui reponde a « combien de familles nommees ce jeu contient-il reellement » — sonder
# des fichiers au hasard mesure la distribution des evenements, pas la couverture.
if [ "$ECH" = "--cles" ]; then
    awk -F/ '{k=$NF; sub(/\.cfg\.bin$/,"",k); sub(/_[0-9.]+$/,"",k);
              if(!(k in v)){v[k]=1; print $0}}' "$TMP/tous" > "$TMP/ech"
    total_cles=$(wc -l < "$TMP/ech")
    export BASE
    sonde() {
        f=$(curl -s --max-time 30 "$BASE/api/v1/donnees/$(printf '%s' "$1" | sed 's/ /%20/g')" \
            | jq -r '.famille // empty' 2>/dev/null)
        [ -n "$f" ] && echo "$f"
        return 0
    }
    export -f sonde
    xargs -a "$TMP/ech" -d '\n' -I{} -P "$PAR" bash -c 'sonde "{}"' > "$TMP/familles"
    echo "corpus  : $(wc -l < "$TMP/tous") fichiers .cfg.bin, $total_cles cles distinctes"
    echo "typees  : $(wc -l < "$TMP/familles") cles rendent une famille nommee"
    echo "familles: $(sort -u "$TMP/familles" | wc -l) distinctes"
    sort "$TMP/familles" | uniq -c | sort -rn | head -20
    exit 0
fi
n=$(wc -l < "$TMP/tous")
pas=$(( n / ECH )); [ "$pas" -ge 1 ] || pas=1
awk -v p="$pas" 'NR % p == 1' "$TMP/tous" > "$TMP/ech"

sonder() {
    corps=$(curl -s --max-time 60 "$1/api/v1/donnees/$(printf '%s' "$2" | sed 's/ /%20/g')")
    f=$(printf '%s' "$corps" | jq -r '.famille // empty' 2>/dev/null || true)
    if [ -n "$f" ]; then echo "TYPE $f"
    elif printf '%s' "$corps" | grep -q 'aucune famille nommee'; then echo "REFUS"
    else echo "AUTRE $(printf '%s' "$corps" | head -c 100)"; fi
}
export -f sonder

xargs -a "$TMP/ech" -I{} -P "$PAR" bash -c 'sonder "$0" "{}"' "$BASE" > "$TMP/res"

types=$(grep -c '^TYPE' "$TMP/res" || true)
refus=$(grep -c '^REFUS' "$TMP/res" || true)
autres=$(grep -c '^AUTRE' "$TMP/res" || true)
total=$(wc -l < "$TMP/res")

echo "corpus  : $n fichiers .cfg.bin, echantillon $total (pas de $pas)"
echo "types   : $types  (famille nommee rendue)"
echo "refus   : $refus  (404 explicite : aucune famille nommee pour cette cle)"
echo "autres  : $autres (ce qui n'est ni l'un ni l'autre — DOIT valoir 0)"
echo
echo "familles distinctes rencontrees :"
grep '^TYPE' "$TMP/res" | awk '{print $2}' | sort | uniq -c | sort -rn | head -30
grep '^AUTRE' "$TMP/res" | head -5 >&2
[ "$autres" -eq 0 ]
