#!/usr/bin/env bash
# Mesure la couverture réelle des huit familles géométriques servies par `nie-site`
# (`/api/v1/formats/decode/{chemin}`, lot 9.1 de `docs/PLAN-SITE-ULTIME.md`).
#
# Ce que ce script prouve, et pourquoi il existe : un décodeur câblé mais jamais interrogé ne
# prouve rien. Il échantillonne l'inventaire VFS figé, interroge le service, et rend pour chaque
# famille un TAUX — jamais un « ça marche ». Un 200 qui rendrait un résumé vide serait compté
# comme un échec : le contrôle porte sur le contenu, pas sur le statut.
#
#   scripts/validation/mesurer-geometrie.sh [base] [echantillon]
#
#   base         URL du service        (défaut http://127.0.0.1:8085)
#   echantillon  fichiers par famille  (défaut 25)
#
# Prérequis : `var/vfs/inventaire.txt` (255 308 lignes, `chemin taille [cpk]`), régénérable par
#   niers vfs find 'data/' -n 300000 > var/vfs/inventaire.txt
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"
N="${2:-25}"
RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INVENTAIRE="$RACINE/var/vfs/inventaire.txt"

[ -f "$INVENTAIRE" ] || { echo "inventaire absent: $INVENTAIRE" >&2; exit 1; }

printf '%-9s %7s %7s %7s %9s  %s\n' famille total testes ok echecs "premier echec"
total_ok=0; total_test=0

for ext in g4pk g4mg objbin g4pkm g4cm col g4sk mevbin g4mt; do
    # Tous les chemins de la famille, puis un pas régulier : un échantillon en tête de fichier
    # ne verrait qu'un seul dossier, donc un seul producteur d'assets.
    mapfile -t tous < <(awk -v e=".$ext" '$1 ~ e"$" {print $1}' "$INVENTAIRE")
    n_total=${#tous[@]}
    [ "$n_total" -gt 0 ] || { echo "$ext: aucun fichier" >&2; continue; }
    pas=$(( n_total / N )); [ "$pas" -lt 1 ] && pas=1

    ok=0; testes=0; premier=""
    for (( i=0; i<n_total && testes<N; i+=pas )); do
        chemin="${tous[$i]}"
        testes=$(( testes + 1 ))
        reponse=$(curl -s -w '\n%{http_code}' --max-time 60 "$BASE/api/v1/formats/decode/$chemin" || echo $'\n000')
        code="${reponse##*$'\n'}"
        corps="${reponse%$'\n'*}"
        # Le contenu, pas le statut : la réponse doit porter le jeton de famille attendu.
        famille=$(printf '%s' "$corps" | jq -r '.resume.famille // empty' 2>/dev/null || true)
        if [ "$code" = "200" ] && [ "$famille" = "$ext" ]; then
            ok=$(( ok + 1 ))
        elif [ -z "$premier" ]; then
            premier="$chemin [$code] $(printf '%s' "$corps" | jq -r '.erreur // .message // empty' 2>/dev/null | head -c 60)"
        fi
    done

    total_ok=$(( total_ok + ok )); total_test=$(( total_test + testes ))
    printf '%-9s %7d %7d %7d %9d  %s\n' ".$ext" "$n_total" "$testes" "$ok" "$(( testes - ok ))" "${premier:-—}"
done

echo
printf 'total: %d/%d decodages conformes (%.1f%%)\n' \
    "$total_ok" "$total_test" "$(awk -v a="$total_ok" -v b="$total_test" 'BEGIN{print (b?100*a/b:0)}')"
[ "$total_ok" = "$total_test" ]
