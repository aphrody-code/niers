#!/usr/bin/env bash
# packager-bases-explorer.sh — prépare les bases QUI VOYAGENT AVEC l'installeur de nie-explorer.
#
# L'explorateur doit servir une expérience complète à une utilisatrice qui n'a ni le dépôt, ni le
# jeu, ni accès au VPS : le miroir du wiki (6 166 personnages, 1 002 techniques, 66 tables
# `inagle_*`) et la base de reverse (117 494 fonctions) sont donc empaquetés dans le bundle Tauri
# (`bundle.resources`), puis décompressés vers `$APPDATA/db/` au premier lancement
# (`installer_bases_embarquees`, src-tauri/src/lib.rs).
#
# Compressées, elles pèsent ~30 Mo pour ~140 Mo de données — d'où gzip plutôt qu'une copie brute.
# Les `.gz` ne sont PAS versionnés (cf. src-tauri/.gitignore) : ils se régénèrent ici, depuis
# `var/`, au moment de la release.
#
# Idempotent : une archive plus récente que sa source n'est pas recompressée.
#
# Usage :
#   scripts/packager-bases-explorer.sh            # régénère ce qui a changé
#   scripts/packager-bases-explorer.sh --force    # recompresse tout
set -euo pipefail

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CIBLE="$RACINE/apps/inacord/src-tauri/resources/db"
FORCE="${1:-}"

mkdir -p "$CIBLE"

# Compresse `$1` (source) vers `$CIBLE/$2.gz` si la source est plus récente que l'archive.
compresser() {
	local source="$1" nom="$2" archive="$CIBLE/$2.gz"
	if [ ! -f "$source" ]; then
		echo "  ✗ $nom : source absente ($source)" >&2
		return 1
	fi
	# `-L` suit le lien : `var/mirror.sqlite` en est un (bascule atomique du miroir).
	local taille
	taille=$(stat -Lc%s "$source")
	if [ "$taille" -lt 1000000 ]; then
		echo "  ✗ $nom : source suspecte ($taille octets) — non empaquetée" >&2
		return 1
	fi
	if [ "$FORCE" != "--force" ] && [ -f "$archive" ] && [ "$archive" -nt "$source" ]; then
		echo "  = $nom : à jour ($(stat -c%s "$archive") octets compressés)"
		return 0
	fi
	echo "  → $nom : compression de $taille octets…"
	gzip -6 -c "$source" >"$archive.part"
	mv -f "$archive.part" "$archive"
	echo "  ✓ $nom : $(stat -c%s "$archive") octets compressés"
}

echo "▸ bases embarquées de nie-explorer → $CIBLE"

# Le miroir du wiki. Sa source canonique est `var/mirror.sqlite` (lien vers l'instantané courant,
# posé par scripts/donnees/miroir-inagle.sh) ; à défaut, le dernier instantané daté.
MIROIR="$RACINE/var/mirror.sqlite"
if [ ! -e "$MIROIR" ]; then
	MIROIR="$(ls -1t "$RACINE"/var/miroir/inagle-*.sqlite 2>/dev/null | head -1 || true)"
fi
compresser "$MIROIR" "mirror.sqlite"

# La base de reverse. Contrairement au miroir, elle se reconstruit sur place (`niers rebuild`) :
# pas de lien, un seul fichier.
compresser "$RACINE/var/niers.sqlite" "niers.sqlite"

# Le catalogue des épisodes de la série (`packages/ietv` → `IETVCache`), que la vue Cinéma
# présente à côté des cinématiques du jeu. 290 Ko : le seuil de validité des deux autres (1 Mo)
# ne s'y applique pas — d'où le contrôle par le nombre d'épisodes plutôt que par la taille.
EPISODES="$RACINE/data/anime/episodes.db"
if [ -f "$EPISODES" ]; then
	NB_EP=$(sqlite3 "$EPISODES" "SELECT count(*) FROM episodes" 2>/dev/null || echo 0)
	if [ "${NB_EP:-0}" -lt 100 ]; then
		echo "  ✗ episodes.db : $NB_EP épisodes — base incomplète, non empaquetée" >&2
		exit 1
	fi
	echo "  → episodes.db : $NB_EP épisodes"
	gzip -6 -c "$EPISODES" >"$CIBLE/episodes.db.gz.part"
	mv -f "$CIBLE/episodes.db.gz.part" "$CIBLE/episodes.db.gz"
	echo "  ✓ episodes.db : $(stat -c%s "$CIBLE/episodes.db.gz") octets compressés"
else
	echo "  ✗ episodes.db : source absente ($EPISODES)" >&2
	exit 1
fi

# Contrôle final : `bundle.resources` porte le glob `resources/db/*.gz`. Un glob qui ne matche
# rien produirait un installeur SANS ses bases — exactement la release qu'on veut éviter, et que
# rien ne distinguerait ensuite d'une release complète.
NB=$(find "$CIBLE" -name '*.gz' -type f | wc -l)
if [ "$NB" -lt 3 ]; then
	echo "ERREUR: $NB archive(s) sur 3 — l'installeur ne serait pas autonome." >&2
	exit 1
fi
echo "  $NB bases prêtes à voyager avec l'installeur ($(du -ch "$CIBLE"/*.gz | tail -1 | cut -f1) au total)"
