#!/usr/bin/env bash
# Mesure la colonne « Servi par l'API » des 48 filtres recenses par docs/FILTRES.md § 5.
#
#   scripts/validation/mesurer-matrice-filtres.sh [base]
#
# ## Pourquoi ce script remplace une colonne tenue a la main
#
# Le recensement du 2026-09-06 a ete fait en LECTURE de code, et il a eu raison de l'etre : il a
# trouve le defaut n°1 du lot 8 (`/b` declarait `q` et l'ignorait). Mais une matrice lue reste
# vraie le jour ou on l'ecrit et faux le lendemain — celle-la annoncait « manquant = 42 » alors
# que `/api/v1/entites/{table}` sert deja `q`, le tri et l'egalite de colonne sur 219 tables.
#
# Ce script ne lit plus le code : il interroge le service monte, et chaque ligne porte sa preuve.
#
# ## Les trois verdicts, et pourquoi le troisieme existe
#
#   SERVI    le filtre REDUIT un total, ou CHANGE un ordre. Un 200 ne suffit pas.
#   ABSENT   le parametre est accepte sans rien changer, ou la route n'existe pas. C'est la
#            moitie negative : sans elle, un script qui rendrait « SERVI » partout passerait.
#   CLIENT   sans objet cote API (une vue en grille, une taille de vignette). Compte a part,
#            jamais comme un manque du serveur.
set -euo pipefail

BASE="${1:-http://127.0.0.1:8085}"

servis=0; absents=0; clients=0; rouges=0

total_de() { curl -s --max-time 60 "$BASE$1" | jq -r '.total // .total_fichiers // .results.total // "ERR"'; }
premier_de() { curl -s --max-time 60 "$BASE$2" | jq -r "$1 // \"ERR\""; }

ligne() { printf '%-5s %-46s %-7s %s\n' "$1" "$2" "$3" "$4"; }

# Le filtre doit faire BAISSER le total. Un parametre avale rendrait le meme nombre.
reduit() { # #  nom  url_nu  url_filtre
    local n="$1" nom="$2" nu="$3" f="$4" a b
    a=$(total_de "$nu"); b=$(total_de "$f")
    if [ "$a" = "ERR" ] || [ "$b" = "ERR" ]; then ligne "$n" "$nom" "ROUGE" "reponse illisible"; rouges=$((rouges+1))
    elif [ "$b" -lt "$a" ]; then ligne "$n" "$nom" "SERVI" "$a -> $b"; servis=$((servis+1))
    else ligne "$n" "$nom" "ROUGE" "non applique ($b >= $a)"; rouges=$((rouges+1)); fi
}

# Le tri ne change aucun total : il change la TETE. C'est la seule mesure qui le distingue d'un
# parametre ignore.
ordonne() { # #  nom  champ_jq  url_asc  url_desc
    local n="$1" nom="$2" ch="$3" x y
    x=$(premier_de "$ch" "$4"); y=$(premier_de "$ch" "$5")
    if [ "$x" = "ERR" ] || [ "$y" = "ERR" ]; then ligne "$n" "$nom" "ROUGE" "reponse illisible"; rouges=$((rouges+1))
    elif [ "$x" != "$y" ]; then ligne "$n" "$nom" "SERVI" "$(printf '%.28s' "$x") / $(printf '%.28s' "$y")"; servis=$((servis+1))
    else ligne "$n" "$nom" "ROUGE" "tri sans effet ($x)"; rouges=$((rouges+1)); fi
}

# La route existe et rend quelque chose — pour les filtres qui SONT la route (une vue, une
# navigation par prefixe), il n'y a pas de total a reduire.
existe() { # #  nom  url  condition_jq
    local n="$1" nom="$2" u="$3" c="$4"
    if curl -s --max-time 60 "$BASE$u" | jq -e "$c" >/dev/null 2>&1
    then ligne "$n" "$nom" "SERVI" "$u"; servis=$((servis+1))
    else ligne "$n" "$nom" "ROUGE" "$u ne satisfait pas $c"; rouges=$((rouges+1)); fi
}

# Moitie negative : le parametre est bien SANS effet. Si un jour il en prend un, cette ligne
# rougit et la matrice doit etre relue — c'est le but.
absent() { # #  nom  url_nu  url_tentative
    local n="$1" nom="$2" nu="$3" t="$4" a b
    a=$(total_de "$nu"); b=$(total_de "$t")
    if [ "$a" = "$b" ]; then ligne "$n" "$nom" "ABSENT" "parametre avale ($a inchange)"; absents=$((absents+1))
    else ligne "$n" "$nom" "NOUVEAU" "$a -> $b : ce filtre EXISTE desormais, relire la matrice"; rouges=$((rouges+1)); fi
}

# La route n'existe pas du tout.
sans_route() { # #  nom  url
    local n="$1" nom="$2" u="$3" code
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 60 "$BASE$3")
    if [ "$code" = "404" ]; then ligne "$n" "$nom" "ABSENT" "404 sur $u"; absents=$((absents+1))
    else ligne "$n" "$nom" "NOUVEAU" "$u repond $code : relire la matrice"; rouges=$((rouges+1)); fi
}

# Une colonne CONSTANTE ou VIDE dans ce gisement ne peut pas reduire un total : la mesurer par
# la reduction rendrait un faux ABSENT. Ce que le service garantit alors est STRUCTUREL — toute
# colonne du schema est filtrable, tout nom hors schema est un 400 — et c'est cela qu'on prouve,
# avec ses deux moities.
accepte() { # #  nom  url_colonne_connue  url_colonne_inventee  note
    local n="$1" nom="$2" bon="$3" faux="$4" note="$5" c1 c2
    c1=$(curl -s -o /dev/null -w '%{http_code}' --max-time 60 "$BASE$bon")
    c2=$(curl -s -o /dev/null -w '%{http_code}' --max-time 60 "$BASE$faux")
    if [ "$c1" = "200" ] && [ "$c2" = "400" ]
    then ligne "$n" "$nom" "SERVI" "colonne acceptee, nom inconnu refuse — $note"; servis=$((servis+1))
    else ligne "$n" "$nom" "ROUGE" "attendu 200/400, obtenu $c1/$c2"; rouges=$((rouges+1)); fi
}

# Le parametre n'est pas avale : il est REFUSE. C'est une absence plus franche que la premiere,
# et le service la doit — un filtre inconnu qui rendrait 200 laisserait croire qu'il a filtre.
absent_400() { # #  nom  url_tentative
    local n="$1" nom="$2" u="$3" code
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 60 "$BASE$u")
    if [ "$code" = "400" ]; then ligne "$n" "$nom" "ABSENT" "refuse en 400, jamais avale"; absents=$((absents+1))
    else ligne "$n" "$nom" "NOUVEAU" "$u repond $code : relire la matrice"; rouges=$((rouges+1)); fi
}

# Une absence qui se demontre par un compte, pas par un parametre : la condition jq DIT ce qui
# manque, et rougira le jour ou ce ne sera plus vrai.
absent_prouve() { # #  nom  url  condition_jq  explication
    local n="$1" nom="$2" u="$3" c="$4" e="$5"
    if curl -s --max-time 60 "$BASE$u" | jq -e "$c" >/dev/null 2>&1
    then ligne "$n" "$nom" "ABSENT" "$e"; absents=$((absents+1))
    else ligne "$n" "$nom" "NOUVEAU" "$u ne verifie plus $c : relire la matrice"; rouges=$((rouges+1)); fi
}

client() { ligne "$1" "$2" "CLIENT" "$3"; clients=$((clients+1)); }

printf '%-5s %-46s %-7s %s\n' '#' filtre verdict preuve
printf '%-5s %-46s %-7s %s\n' ----- '---------------------------------------------' ------- ------

echo "-- Fichiers / VFS"
reduit  1 "sous-chaine sur le chemin" \
        "/api/v1/recherche?per_page=1" "/api/v1/recherche?per_page=1&q=chara_base"
reduit  2 "sous-chaine dans le parcours /b" \
        "/b/data/common/gamedata/character?per_page=1" \
        "/b/data/common/gamedata/character?per_page=1&q=chara_base"
reduit  3 "extension exacte" \
        "/api/v1/recherche?per_page=1" "/api/v1/recherche?per_page=1&ext=g4tx"
existe  4 "famille d'asset (vues)" "/api/v1/textures?per_page=1" '.total > 0'
existe  5 "navigation par prefixe" "/b/data/common?per_page=1" '(.dossiers|length) > 0'
absent_prouve 6 "recherche restreinte a un sous-arbre" \
        "/b/data/common/gamedata?per_page=1&q=chara" '.total_fichiers == 0' \
        "/b filtre le dossier DIRECT, pas le sous-arbre : 0 ici, des milliers plus bas"
ordonne 7 "tri par nom" '.fichiers[0].nom' \
        "/api/v1/recherche?per_page=1&tri=nom&ordre=asc" \
        "/api/v1/recherche?per_page=1&tri=nom&ordre=desc"
ordonne 8 "tri par taille" '.fichiers[0].taille' \
        "/api/v1/recherche?per_page=1&tri=taille&ordre=asc" \
        "/api/v1/recherche?per_page=1&tri=taille&ordre=desc"
reduit  9 "taille min / max" \
        "/api/v1/recherche?per_page=1" "/api/v1/recherche?per_page=1&taille_min=1000000"
reduit 10 "CPK d'origine" \
        "/api/v1/recherche?per_page=1" \
        "/api/v1/recherche?per_page=1&cpk=672c0647c5ff4adf150dc88695184817.cpk"
absent 11 "glob (**, !excl, listes)" \
        "/api/v1/recherche?per_page=1" "/api/v1/recherche?per_page=1&glob=data/**/*.g4tx"
client 12 "vue liste / grille" "affichage, aucune donnee serveur"
client 13 "taille de vignette" "affichage, aucune donnee serveur"
existe 14 "per_page reglable" "/api/v1/recherche?per_page=7" '.per_page == 7'
existe 15 "etat de filtre dans l'URL" "/api/v1/recherche?per_page=1&q=chara&ext=bin" '.filtres.q == "chara"'
existe 16 "compte total affiche" "/b/data/common?per_page=1" '.total_fichiers != null'

echo "-- Catalogue de personnages"
reduit 17 "nom FR / EN / JA" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&q=mark"
reduit 18 "element" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&element=Feu"
reduit 19 "poste" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&position=Attaquant"
reduit 20 "rarete" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&rarity=2"
reduit 21 "serie" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&series=Victory%20Road"
reduit 22 "genre" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&gender=M"
existe 23 "style de jeu (6)" "/api/v1/playstyles/0?per_page=1" '.total > 0'
accepte 24 "tranche d'age" \
        "/api/v1/entites/inagle_characters?per_page=1&age_group=1" \
        "/api/v1/entites/inagle_characters?per_page=1&age_groupe=1" \
        "age_group est VIDE sur les 6 166 lignes de ce gisement"
reduit 25 "equipe" \
        "/api/v1/entites/inagle_characters?per_page=1" \
        "/api/v1/entites/inagle_characters?per_page=1&team_id=0xA1C76AAD"
existe 26 "role (coach / coordinator)" "/api/v1/entites/inagle_coordinators?per_page=1" '.total > 0'
ordonne 27 "tri du catalogue" '.elements[0].id' \
        "/api/v1/entites/inagle_characters?per_page=1&tri=name_fr&ordre=asc" \
        "/api/v1/entites/inagle_characters?per_page=1&tri=name_fr&ordre=desc"

echo "-- Techniques / objets / autres catalogues"
reduit 28 "categorie de technique" \
        "/api/v1/entites/inagle_skills?per_page=1" \
        "/api/v1/entites/inagle_skills?per_page=1&category=Tir"
absent_prouve 29 "presence d'une video" \
        "/api/v1/entites/inagle_skills?per_page=1&has_telop=1" '.total == 1002' \
        "l'egalite ne sait pas dire NON NUL, et has_telop vaut 1 sur les 1 002"
accepte 30 "hyper / aura" \
        "/api/v1/entites/inagle_skills?per_page=1&is_hyper=1" \
        "/api/v1/entites/inagle_skills?per_page=1&is_hyperr=1" \
        "is_hyper vaut 0 sur les 1 002 lignes de ce gisement"
accepte 31 "eldorado (dit overdrive cote wiki)" \
        "/api/v1/entites/inagle_skills?per_page=1&is_eldorado=1" \
        "/api/v1/entites/inagle_skills?per_page=1&is_eldoradoo=1" \
        "is_eldorado vaut 0 sur les 1 002 lignes de ce gisement"
absent_400 32 "fourchette numerique (puissance 0->880)" \
        "/api/v1/entites/inagle_skills?per_page=1&power_max_min=400"
ordonne 33 "tri par puissance / cout" '.elements[0].power_max' \
        "/api/v1/entites/inagle_skills?per_page=1&tri=power_max&ordre=asc" \
        "/api/v1/entites/inagle_skills?per_page=1&tri=power_max&ordre=desc"
reduit 34 "categorie d'objet" \
        "/api/v1/entites/inagle_items?per_page=1" \
        "/api/v1/entites/inagle_items?per_page=1&category=emblem"
reduit 35 "categorie d'illustration" \
        "/api/v1/entites/inagle_gallery?per_page=1" \
        "/api/v1/entites/inagle_gallery?per_page=1&flg_no=1"
absent 36 "langue / variante d'un asset" \
        "/api/v1/recherche?per_page=1" "/api/v1/recherche?per_page=1&langue=fr"

echo "-- Episodes / medias"
absent 37 "saison / numero d'episode" \
        "/api/v1/episodes?limit=5000" "/api/v1/episodes?limit=5000&season=3"
absent 38 "langue de piste" \
        "/api/v1/episodes?limit=5000" "/api/v1/episodes?limit=5000&language=fr"
absent 39 "sous-titres presents / vu" \
        "/api/v1/episodes?limit=5000" "/api/v1/episodes?limit=5000&subtitles=1"
absent 40 "classement par pertinence ponderee" \
        "/api/v1/episodes?limit=5000" "/api/v1/episodes?limit=5000&q=match"
absent 41 "repli approche (fuzzy)" \
        "/api/v1/recherche?per_page=1&q=charabase" "/api/v1/recherche?per_page=1&q=charabase&fuzzy=1"

echo "-- 3D"
# Sans `famille`, le catalogue 3D ne rend pas TOUT : il rend `perso` (5 490 = le total nu,
# alors que les six familles somment a 6 191). Mesurer avec `famille=perso` rendrait donc le
# meme nombre et ferait passer un filtre servi pour un filtre avale.
reduit 42 "famille de modele" \
        "/api/v1/3d/modeles?per_page=1" "/api/v1/3d/modeles?per_page=1&famille=waza"
reduit 43 "recherche code ou nom" \
        "/api/v1/3d/modeles?per_page=1" "/api/v1/3d/modeles?per_page=1&q=c01"

echo "-- Reverse / forge"
sans_route 44 "recherche fonction par nom ou adresse" "/api/v1/re/functions"
sans_route 45 "filtre par statut de forge" "/api/v1/forge/units"

echo "-- Transverse"
absent 46 "recherche globale multi-gisements" \
        "/api/v1/recherche?per_page=1&q=mark" "/api/v1/recherche?per_page=1&q=mark&gisements=tous"
existe 47 "facettes avec comptes" "/api/v1/playstyles" '[.playstyles[].characters]|add > 0'
absent_400 48 "export de la liste filtree" \
        "/api/v1/entites/inagle_characters?per_page=1&q=mark&format=csv"

echo
printf 'servis %d · absents %d · cote client %d · a relire %d  (sur %d)\n' \
    "$servis" "$absents" "$clients" "$rouges" $(( servis + absents + clients + rouges ))
[ "$rouges" -eq 0 ]
