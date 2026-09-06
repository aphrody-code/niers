#!/usr/bin/env bash
# Mesure le decodage des NEUF familles que la matrice de couverture classait `bloque` — c'est-a-
# dire « aucun parseur, du reverse est necessaire » — alors que le depot les decodait deja.
#
#   scripts/validation/mesurer-formats-bloques.sh [base] [par_famille]
#
#   base         URL du service            (defaut http://127.0.0.1:8085)
#   par_famille  echantillon par extension (defaut 25)
#
# ## Pourquoi ce script existe
#
# C'est la QUATRIEME fois que ce depot classe `bloque` un format qu'il sait lire (§ 9 bis du cap
# pour `.g4ma`/`.g4vs`/`.g4la`, puis les `.bin`, puis les shaders). Le defaut se repete parce
# que le classement se fait sur l'EXTENSION et que la lecture se fait sur le MAGIC : un
# `.pfxo` ressortait « ni magic connu » en publiant `44 58 42 43` — c'est-a-dire `DXBC` en
# ASCII. Le message d'erreur portait la refutation de ce qu'il affirmait.
#
# Ce script est la garde qui empeche la cinquieme fois : il interroge la route montee, sur un
# echantillon reel, et exige le JETON de format dans le corps. Un code 200 ne compte pas —
# `/api/v1/formats/decode` repond aussi en disant « format non identifie ».
#
# L'echantillon est pris a PAS REGULIER sur la liste triee, jamais en tete : les premiers
# fichiers d'un dossier se ressemblent, et un echantillon de tete mesure un dossier, pas un
# corpus.
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"
PAR_FAMILLE="${2:-25}"
RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INVENTAIRE="$RACINE/var/vfs/inventaire.txt"

[ -f "$INVENTAIRE" ] || { echo "inventaire absent: $INVENTAIRE" >&2; exit 1; }

# Le chemin se lit en retirant les DEUX derniers champs (taille, cpk) : des chemins du VFS
# contiennent un espace, et un decoupage par espaces croissants les casse.
CHEMINS=$(mktemp); trap 'rm -f "$CHEMINS"' EXIT
sed -E 's/ [0-9]+ \[[^]]*\]$//; s/ [0-9]+ \[?[^ ]*\]?$//' "$INVENTAIRE" > "$CHEMINS"

# extension:jeton attendu. Le jeton n'est PAS l'extension : c'est ce que le decodeur reconnait,
# et l'ecart entre les deux est precisement ce que ce script mesure.
FAMILLES="vfxo:dxbc pfxo:dxbc cfxo:dxbc gfxo:dxbc fxbin:t2b ptlb:t2b clobin:t2b linb:t2b bin:rdbn|t2b"

total=0; ok=0
printf '%-8s %-9s %7s %7s  %s\n' famille jeton testes reussis exemple

for paire in $FAMILLES; do
    ext="${paire%%:*}"
    jetons="${paire##*:}"
    # `.bin` exclut les deux extensions composees, qui ont leurs propres familles.
    if [ "$ext" = "bin" ]; then
        liste=$(grep '\.bin$' "$CHEMINS" | grep -vE '\.cfg\.bin$|\.lua\.bin$' || true)
    else
        liste=$(grep "\.$ext\$" "$CHEMINS" || true)
    fi
    n=$(printf '%s\n' "$liste" | grep -c . || true)
    [ "$n" -gt 0 ] || { printf '%-8s %-9s %7s %7s  %s\n' "$ext" "$jetons" 0 0 "aucun fichier"; continue; }
    pas=$(( n / PAR_FAMILLE )); [ "$pas" -ge 1 ] || pas=1

    f_total=0; f_ok=0; exemple=""
    while IFS= read -r chemin; do
        [ -n "$chemin" ] || continue
        f_total=$(( f_total + 1 ))
        corps=$(curl -s --max-time 60 \
            "$BASE/api/v1/formats/decode/$(printf '%s' "$chemin" | sed 's/ /%20/g')" || true)
        if printf '%s' "$corps" | jq -e --arg j "$jetons" '.format as $f | ($j | split("|")) | index($f) != null' >/dev/null 2>&1; then
            f_ok=$(( f_ok + 1 ))
            [ -n "$exemple" ] || exemple=$(printf '%s' "$corps" | jq -c '.produit // .resume // .format' | head -c 70)
        else
            echo "  ECHEC $chemin -> $(printf '%s' "$corps" | head -c 130)" >&2
        fi
    done < <(printf '%s\n' "$liste" | awk -v p="$pas" 'NR % p == 1 || p == 1')

    printf '%-8s %-9s %7s %7s  %s\n' "$ext" "$jetons" "$f_total" "$f_ok" "$exemple"
    total=$(( total + f_total )); ok=$(( ok + f_ok ))
done

echo
printf 'total: %d/%d decodes (%d%%)\n' "$ok" "$total" $(( total > 0 ? ok * 100 / total : 0 ))
[ "$ok" -eq "$total" ]
