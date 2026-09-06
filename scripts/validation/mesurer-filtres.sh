#!/usr/bin/env bash
# Mesure les FILTRES que l'API d'Aphrody applique reellement — lot 8 de docs/PLAN-SITE-ULTIME.md.
#
#   scripts/validation/mesurer-filtres.sh [base]
#
# ## Ce qui est verifie, et pourquoi ce n'est pas un code HTTP
#
# Un filtre servi mais jamais applique compte comme manquant : c'est le defaut n°1 du lot 8,
# `/b` acceptait `q` et l'ignorait. Chaque ligne ci-dessous exige donc **une reduction du
# total**, pas un 200 :
#
#   total(sans filtre)  >  total(avec filtre discriminant)  >=  total(avec filtre absurde)
#
# et la troisieme colonne verifie qu'un filtre absurde rend bien 0. Une implementation qui
# ignorerait le parametre rendrait trois fois le meme nombre et rougirait ici.
#
# La reponse doit en plus REPUBLIER le filtre applique (champ `filtres`) : sans lui, un client
# ne peut pas distinguer « filtre applique » de « filtre avale ».
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"

total_de() { # $1 = chemin+query ; lit `total` ou `total_fichiers`
    curl -s --max-time 60 "$BASE$1" | jq -r '.total // .total_fichiers // .results.total // "ERR"'
}
# Les routes republient le filtre applique sous trois formes selon leur DTO : un bloc `filtres`
# (`/api/v1/recherche`, `/b`, `/api/v1/entites`), un champ `q` de premier niveau (les catalogues
# qui ont leur propre enveloppe), ou le `q` de la `Page` partagee. Les trois valent — ce qui ne
# vaut pas, c'est de n'en publier aucune : le client ne peut alors pas distinguer un filtre
# applique d'un filtre avale.
publie_filtres() { # $1 = chemin+query
    curl -s --max-time 60 "$BASE$1" \
        | jq -e '(.filtres // .results.filtres // .q // .results.q) != null' >/dev/null 2>&1
}

ok=0; ko=0
# `absurde` vide = pas de valeur absurde possible pour ce filtre. C'est le cas des bornes de
# taille : `taille_max=0` retient legitimement les fichiers de zero octet (il y en a), et
# `taille_min` hors u32 est refuse en 400 par la deserialisation — ce qui est le bon
# comportement, pas un filtre qui ne filtre pas. La reduction reste exigee dans les deux cas.
verifier() { # nom | chemin_nu | chemin_discriminant | [chemin_absurde]
    local nom="$1" nu="$2" disc="$3" absurde="${4:-}"
    local a b c="-"
    a=$(total_de "$nu"); b=$(total_de "$disc")
    [ -n "$absurde" ] && c=$(total_de "$absurde")
    local verdict="OK"
    if [ "$a" = "ERR" ] || [ "$b" = "ERR" ] || [ "$c" = "ERR" ]; then verdict="ERREUR"
    elif [ "$b" -ge "$a" ]; then verdict="NON APPLIQUE ($b >= $a)"
    elif [ -n "$absurde" ] && [ "$c" -ne 0 ]; then verdict="ABSURDE NON VIDE ($c)"
    elif ! publie_filtres "$disc"; then verdict="FILTRE NON REPUBLIE"
    fi
    printf '%-34s %9s %9s %8s  %s\n' "$nom" "$a" "$b" "$c" "$verdict"
    if [ "$verdict" = "OK" ]; then ok=$(( ok + 1 )); else ko=$(( ko + 1 )); fi
}

printf '%-34s %9s %9s %8s  %s\n' filtre sans avec absurde verdict

# ── L'espace VFS ────────────────────────────────────────────────────────────────────────────
verifier "recherche: sous-chaine (q)" \
    "/api/v1/recherche?per_page=1" \
    "/api/v1/recherche?per_page=1&q=chara_base" \
    "/api/v1/recherche?per_page=1&q=zzzzaucunfichier"
verifier "recherche: extension (ext)" \
    "/api/v1/recherche?per_page=1" \
    "/api/v1/recherche?per_page=1&ext=g4tx" \
    "/api/v1/recherche?per_page=1&ext=zzzz"
verifier "recherche: taille minimale" \
    "/api/v1/recherche?per_page=1" \
    "/api/v1/recherche?per_page=1&taille_min=1000000"
verifier "recherche: taille maximale" \
    "/api/v1/recherche?per_page=1" \
    "/api/v1/recherche?per_page=1&taille_max=64"
verifier "parcours /b: sous-chaine (q)" \
    "/b/data/common/gamedata/character?per_page=1" \
    "/b/data/common/gamedata/character?per_page=1&q=chara_base" \
    "/b/data/common/gamedata/character?per_page=1&q=zzzzaucunfichier"
# Un dossier a extensions MIXTES, sinon `ext=` retient tout et le test ne prouve rien : sur
# `gamedata/menu/obj`, les 3 373 fichiers sont des `.objbin`, et `ext=objbin` rend donc
# legitimement 3 373. Un test qui ne peut pas echouer ne mesure rien (§ 3 du cap).
verifier "parcours /b: extension (ext)" \
    "/b/data/common/menu/91_quest/quest02/quest02_03?per_page=1" \
    "/b/data/common/menu/91_quest/quest02/quest02_03?per_page=1&ext=g4mg" \
    "/b/data/common/menu/91_quest/quest02/quest02_03?per_page=1&ext=zzzz"

# ── Les gisements ───────────────────────────────────────────────────────────────────────────
verifier "entites: sous-chaine (q)" \
    "/api/v1/entites/inagle_characters?per_page=1" \
    "/api/v1/entites/inagle_characters?per_page=1&q=mark" \
    "/api/v1/entites/inagle_characters?per_page=1&q=zzzzaucunnom"
verifier "entites: egalite de colonne" \
    "/api/v1/entites/inagle_characters?per_page=1" \
    "/api/v1/entites/inagle_characters?per_page=1&element=Feu" \
    "/api/v1/entites/inagle_characters?per_page=1&element=Zzzz"

# ── Le texte localise ───────────────────────────────────────────────────────────────────────
# Le motif discriminant doit RETENIR quelque chose : `q=tornade` rendait 0, et une route qui
# repondrait toujours 0 aurait passe la ligne. `tir` retient 60 des 2 755 entrees — le total
# baisse sans s'effondrer, ce qui est la seule forme qui distingue un filtre d'une panne.
verifier "text: sous-chaine par langue" \
    "/api/v1/text/fr/menu_text?per_page=1" \
    "/api/v1/text/fr/menu_text?per_page=1&q=tir" \
    "/api/v1/text/fr/menu_text?per_page=1&q=zzzzaucuntexte"

# ── Les capacites cablees le 2026-09-06 ─────────────────────────────────────────────────────
verifier "passifs: sous-chaine (q)" \
    "/api/v1/passives/player?per_page=1" \
    "/api/v1/passives/player?per_page=1&q=tir" \
    "/api/v1/passives/player?per_page=1&q=zzzzaucunpassif"
verifier "icones: sous-chaine (q)" \
    "/api/v1/icons?per_page=1" \
    "/api/v1/icons?per_page=1&q=abl" \
    "/api/v1/icons?per_page=1&q=zzzzaucuneicone"
verifier "ecrans: sous-chaine (q)" \
    "/api/v1/screens?per_page=1" \
    "/api/v1/screens?per_page=1&q=menu" \
    "/api/v1/screens?per_page=1&q=zzzzaucunecran"
verifier "familles: sous-chaine (q)" \
    "/api/v1/donnees/familles?per_page=1" \
    "/api/v1/donnees/familles?per_page=1&q=chara" \
    "/api/v1/donnees/familles?per_page=1&q=zzzzaucunefamille"
verifier "modes: sous-chaine (q)" \
    "/api/v1/modes?per_page=1" \
    "/api/v1/modes?per_page=1&q=victory" \
    "/api/v1/modes?per_page=1&q=zzzzaucunmode"

echo
printf 'total: %d/%d filtres reellement appliques\n' "$ok" $(( ok + ko ))
[ "$ko" -eq 0 ]
