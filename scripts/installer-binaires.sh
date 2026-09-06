#!/usr/bin/env bash
# Publie dans ~/.local/bin tous les exécutables du dépôt — Rust et CLI Bun — et déduplique.
#
# DÉDUPLICATION : des liens symboliques vers target/release, jamais des copies. 178 Mio de
# binaires (dont nie-editor à 82 Mio) ne sont donc écrits qu'une fois, et un `cargo build
# --release` met à jour la commande publiée sans réinstallation. Une copie, elle, se périme en
# silence — le pire défaut possible pour un dépôt qui mesure des octets.
#
# COLLISIONS : un lien n'est jamais posé par-dessus un exécutable étranger déjà dans le PATH.
# Vécu le 2026-09-02 : ast-grep pose un alias `sg` qui masque `setgroup` (util-linux). Ici on
# refuse et on le dit, plutôt que d'écraser une commande système.
#
# Usage : bash scripts/installer-binaires.sh [--dry-run]
set -u

# MSYS/Git Bash n'émet PAS de lien symbolique par défaut : `ln -s` y COPIE le fichier, avec
# exit 0 et sans un mot. Mesuré le 2026-09-06 sur le poste Windows : ~/.local/bin/niers.exe
# était un vrai fichier de 27 690 496 octets, `readlink` vide — exactement la copie périmable
# que l'en-tête ci-dessus dit refuser. `winsymlinks:nativestrict` demande le lien natif ET fait
# échouer `ln` si le système ne l'accorde pas, au lieu de retomber en silence sur la copie.
# Vérifié ici : lien natif obtenu, exit 0. Sur une machine sans mode développeur ni privilège
# SeCreateSymbolicLink, `ln` échouera bruyamment — c'est le comportement voulu, et `verifier_lien`
# ci-dessous le rattrape dans tous les cas.
export MSYS=winsymlinks:nativestrict
export CYGWIN=winsymlinks:nativestrict

cd "$(dirname "$0")/.." || exit 1
racine=$PWD
dest=${NIERS_BIN_DIR:-$HOME/.local/bin}
sec=${1:-}

mkdir -p "$dest"
poses=0
sautes=0
refuses=0
copies=0

lien() { # $1 = nom publié, $2 = cible absolue
    local nom=$1 cible=$2 actuel
    [ -e "$cible" ] || { printf '  ??  %-20s cible absente (%s)\n' "$nom" "${cible#$racine/}"; return; }
    actuel=$(command -v "$nom" 2>/dev/null || true)
    if [ -n "$actuel" ] && [ "$actuel" != "$dest/$nom" ]; then
        printf '  !!  %-20s REFUSÉ — %s existe déjà dans le PATH\n' "$nom" "$actuel"
        refuses=$((refuses + 1))
        return
    fi
    if [ "$(readlink -f "$dest/$nom" 2>/dev/null || true)" = "$(readlink -f "$cible")" ]; then
        sautes=$((sautes + 1))
        return
    fi
    if [ "$sec" != "--dry-run" ]; then
        if ! ln -sfn "$cible" "$dest/$nom"; then
            printf '  !!  %-20s ÉCHEC du lien symbolique\n' "$nom"
            refuses=$((refuses + 1))
            return
        fi
        # On ne CROIT pas `ln` sur parole : la contrainte de l'en-tête est « un lien, jamais une
        # copie », et c'est cette propriété-là qu'on mesure. Un exit 0 ayant produit un fichier
        # réel est le faux vert que ce script existe pour éviter.
        if [ ! -L "$dest/$nom" ]; then
            printf '  !!  %-20s COPIE et non lien — elle se périmera en silence\n' "$nom"
            copies=$((copies + 1))
            return
        fi
    fi
    printf '  ->  %-20s %s\n' "$nom" "${cible#$racine/}"
    poses=$((poses + 1))
}

echo "Binaires Rust (target/release) :"
for f in target/release/*; do
    [ -f "$f" ] && [ -x "$f" ] || continue
    case $f in *.d | *.so | *.rlib) continue ;; esac
    lien "$(basename "$f")" "$racine/$f"
done

# Les CLI Bun ne sont pas des exécutables : on publie un lanceur. `bun --bun` est obligatoire —
# `bun run` honore le shebang `#!/usr/bin/env node`, et node est proscrit ici.
echo
echo "CLI Bun (lanceurs) :"
for spec in \
    "nie-catalog:packages/nie-catalog/src/cli.ts" \
    "niers-azalee:packages/azalee-tools/src/cli.ts" \
    "niers-inagle:packages/inagle/src/cli.ts" \
    "niers-mcp:packages/mcp/src/cli.ts" \
    "niers-bxc:apps/bxc/src/cli.ts"; do
    nom=${spec%%:*}
    src=${spec#*:}
    [ -f "$src" ] || { printf '  ??  %-20s source absente (%s)\n' "$nom" "$src"; continue; }
    actuel=$(command -v "$nom" 2>/dev/null || true)
    if [ -n "$actuel" ] && [ "$actuel" != "$dest/$nom" ]; then
        printf '  !!  %-20s REFUSÉ — %s existe déjà\n' "$nom" "$actuel"
        refuses=$((refuses + 1))
        continue
    fi
    # Le lanceur se place À LA RACINE du dépôt. Mesuré le 2026-09-02 : lancée depuis /tmp,
    # `nie-catalog etat` annonce « extrait : 0 tables » et « re : aucune mesure » — ses gisements
    # (var/mirror.sqlite, var/niers.sqlite) sont résolus relativement au cwd. Sans ce cd, une CLI
    # publiée globalement rapporte des gisements VIDES au lieu d'une erreur : un faux négatif.
    if [ "$sec" != "--dry-run" ]; then
        printf '#!/usr/bin/env bash\ncd "%s" || exit 1\nexec bun --bun "%s" "$@"\n' \
            "$racine" "$src" > "$dest/$nom"
        chmod +x "$dest/$nom"
    fi
    printf '  ->  %-20s %s\n' "$nom" "$src"
    poses=$((poses + 1))
done

echo
printf '%d posés, %d déjà à jour, %d refusés (collision), %d copies → %s\n' \
    "$poses" "$sautes" "$refuses" "$copies" "$dest"
if [ "$copies" -gt 0 ]; then
    echo
    echo "ATTENTION : $copies commande(s) publiée(s) en COPIE. Elles ne suivront pas un"
    echo "\`cargo build --release\` et se périmeront sans le dire. Sous Windows, activez le mode"
    echo "développeur (ou accordez SeCreateSymbolicLinkPrivilege) et relancez."
fi
echo
echo "Rappel de doctrine : \`niers\` est la seule CLI utilisateur. \`nie-mem\` et \`nie-steam\`"
echo "recouvrent \`niers mem\` et \`niers steam\` — publiés pour l'outillage, mais une commande"
echo "nouvelle s'écrit dans nie-cli, jamais dans un binaire de plus."
