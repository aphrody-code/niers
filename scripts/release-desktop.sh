#!/usr/bin/env bash
# release-desktop.sh — pipeline de release COMPLET pour l'app desktop Inacord (ex nie-explorer, identifiant Tauri conserve).
#   bump versions → sync lockfiles → build signé (msi+nsis) → tag+push → GitHub Release
#   → (option) redeploy azalee.
#
# Remplace la séquence manuelle du 2026-08-08 (bump Cargo.toml/package.json à la main,
# `cargo update --workspace`, `bun install`, `bunx tauri signer generate`, build, `gh release
# create` avec upload manuel des 5 assets) par UNE commande idempotente et rejouable.
#
# Usage :
#   TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/niers.key ./scripts/release-desktop.sh 0.5.0
#   ./scripts/release-desktop.sh 0.5.0 --ship-azalee   # + redeploy azalee (rare, cf. NOTE ci-dessous)
#
# NOTE — le côté VPS n'a PAS besoin d'être redéployé à chaque release : `azalee.rosegriffon.fr/
# tools/niers` et `/tools/niers/latest.json` lisent la dernière release GitHub EN DIRECT
# (`apps/azalee/lib/niers-releases.ts`, revalidate=3600s) — ce script suffit à lui seul à publier
# une version que l'updater Tauri ET la page de download verront sous 1h max, sans toucher au VPS.
# `--ship-azalee` ne sert que si le CODE d'azalee (pas niers) a aussi changé entre-temps.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-}"
SHIP_AZALEE=0
for arg in "$@"; do [ "$arg" = "--ship-azalee" ] && SHIP_AZALEE=1; done
if [ -z "$VERSION" ] || [[ "$VERSION" == --* ]]; then
	echo "Usage: $0 <version, ex: 0.5.0> [--ship-azalee]" >&2
	exit 1
fi
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
	echo "ERREUR: version attendue au format X.Y.Z (reçu: $VERSION)" >&2
	exit 1
fi
TAG="v$VERSION"

# ── 0. Garde-fous ────────────────────────────────────────────────────────────────────────
[ -z "$(git status --porcelain)" ] || { echo "ERREUR: arbre de travail non propre — commit/stash d'abord." >&2; exit 1; }
[ "$(git rev-parse --abbrev-ref HEAD)" = "main" ] || { echo "ERREUR: doit être sur main (workflow main direct, cf. CLAUDE.md)." >&2; exit 1; }
git rev-parse "$TAG" >/dev/null 2>&1 && { echo "ERREUR: le tag $TAG existe déjà." >&2; exit 1; }
command -v gh >/dev/null || { echo "ERREUR: gh CLI introuvable." >&2; exit 1; }
KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/niers.key}"
[ -f "$KEY_PATH" ] || {
	echo "ERREUR: clé de signature absente ($KEY_PATH)." >&2
	echo "  Génère-la une fois avec : bunx tauri signer generate -w $KEY_PATH --ci" >&2
	echo "  Puis colle la clé publique dans apps/inacord/src-tauri/tauri.conf.json (plugins.updater.pubkey)." >&2
	exit 1
}

echo "▸ [1/8] bump version → $VERSION (workspace Cargo + Bun)…"
sed -i "s/^version = \"[0-9]*\.[0-9]*\.[0-9]*\"/version = \"$VERSION\"/" Cargo.toml
sed -i "s/\"version\": \"[0-9]*\.[0-9]*\.[0-9]*\"/\"version\": \"$VERSION\"/" package.json
for f in apps/inacord/package.json apps/nie-mcp/package.json \
         packages/nie/package.json packages/nie-bridge/package.json \
         packages/nie-plugin/package.json; do
	[ -f "$f" ] && sed -i "s/\"version\": \"[0-9]*\.[0-9]*\.[0-9]*\"/\"version\": \"$VERSION\"/" "$f"
done
sed -i "s/^version = \"[0-9]*\.[0-9]*\.[0-9]*\"/version = \"$VERSION\"/" apps/inacord/src-tauri/Cargo.toml
sed -i "s/\"version\": \"[0-9]*\.[0-9]*\.[0-9]*\"/\"version\": \"$VERSION\"/" apps/inacord/src-tauri/tauri.conf.json

echo "▸ [2/8] sync lockfiles (Cargo.lock + bun.lock)…"
cargo update --workspace --offline 2>/dev/null || cargo update --workspace
(cd apps/inacord/src-tauri && cargo update --workspace --offline 2>/dev/null || cargo update --workspace)
bun install

echo "▸ [3/8] sanity check (cargo check workspace + src-tauri)…"
cargo check --workspace
(cd apps/inacord/src-tauri && cargo check)

echo "▸ [4/8] zip extension Blender (plugins/niers-blender, hors __pycache__)…"
BLENDER_VERSION="$(grep -m1 '^version' plugins/niers-blender/blender_manifest.toml | sed -E 's/.*"([0-9.]+)".*/\1/')"
ZIP_STAGE="$(mktemp -d)"
mkdir -p "$ZIP_STAGE/niers"   # racine = nom de MODULE Python (le dossier source a un tiret)
cp -r plugins/niers-blender/. "$ZIP_STAGE/niers/"
find "$ZIP_STAGE" -iname "__pycache__" -type d -exec rm -rf {} + 2>/dev/null || true
BLENDER_ZIP="$ROOT/apps/inacord/src-tauri/target/release/bundle/niers-$BLENDER_VERSION.zip"
# `zip` n'existe pas sur une install Windows standard (ni Git Bash, ni MSYS ne le fournissent) :
# repli sur Compress-Archive, présent partout où PowerShell l'est. Sans ce repli, la release
# s'arrêtait ici alors que tout le reste était prêt.
if command -v zip >/dev/null; then
	(cd "$ZIP_STAGE" && zip -qr "$BLENDER_ZIP" niers)
elif command -v powershell >/dev/null; then
	# Compress-Archive refuse d'écraser sans -Force et veut des chemins Windows.
	WIN_STAGE="$(cd "$ZIP_STAGE" && pwd -W 2>/dev/null || echo "$ZIP_STAGE")"
	WIN_ZIP="$(cd "$(dirname "$BLENDER_ZIP")" && pwd -W 2>/dev/null || dirname "$BLENDER_ZIP")/$(basename "$BLENDER_ZIP")"
	powershell -NoProfile -Command 		"Compress-Archive -Path '$WIN_STAGE/niers' -DestinationPath '$WIN_ZIP' -Force" >/dev/null
else
	echo "ERREUR: ni zip ni powershell disponibles pour empaqueter l'extension Blender." >&2
	exit 1
fi
rm -rf "$ZIP_STAGE"
echo "  → $BLENDER_ZIP (addon v$BLENDER_VERSION)"

echo "▸ [5/8] bases embarquées (miroir wiki + base RE → resources/db/*.gz)…"
# Ce que l'installeur emporte pour être utile SANS le jeu et SANS le dépôt. L'étape est ici, avant
# le build : `bundle.resources` est lu par le bundler, une archive écrite après coup n'entrerait
# dans aucun paquet. Le script s'arrête si une base manque — une release amputée de ses données
# s'installe et se signe exactement comme une release complète, rien ne l'en distinguerait ensuite.
"$ROOT/scripts/packager-bases-explorer.sh"

echo "▸ [6/8] build desktop signé (msi + nsis, minisign)…"
(
	cd apps/inacord
	export TAURI_SIGNING_PRIVATE_KEY="$KEY_PATH"
	export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
	bun run tauri build
)
BUNDLE="apps/inacord/src-tauri/target/release/bundle"
MSI="$BUNDLE/msi/Inacord_${VERSION}_x64_en-US.msi"
NSIS="$BUNDLE/nsis/Inacord_${VERSION}_x64-setup.exe"
for f in "$MSI" "$MSI.sig" "$NSIS" "$NSIS.sig"; do
	[ -f "$f" ] || { echo "ERREUR: artefact attendu absent: $f" >&2; exit 1; }
done

# Un installeur peut exister, etre signe, et ne PAS contenir l'application. C'est arrive : le
# bundler empaquetait `export-bindings.exe` (182 Ko) a la place du binaire (30 Mo), avec une
# signature minisign parfaitement valide — rien dans la chaine updater ne l'aurait refuse.
# Le seul controle qui l'attrape est la taille : l'app pese des dizaines de Mo, un paquet vide
# quelques centaines de Ko.
MIN_MSI_BYTES=5000000
MIN_NSIS_BYTES=3000000
msi_size=$(wc -c <"$MSI")
nsis_size=$(wc -c <"$NSIS")
[ "$msi_size" -ge "$MIN_MSI_BYTES" ] || {
	echo "ERREUR: le MSI ne fait que $msi_size octets (minimum $MIN_MSI_BYTES)." >&2
	echo "  L'application n'y est probablement pas — verifier qu'un seul binaire est construit" >&2
	echo "  en release (cf. required-features de export-bindings dans src-tauri/Cargo.toml)." >&2
	exit 1
}
[ "$nsis_size" -ge "$MIN_NSIS_BYTES" ] || {
	echo "ERREUR: l'installeur NSIS ne fait que $nsis_size octets (minimum $MIN_NSIS_BYTES)." >&2
	exit 1
}
echo "  taille verifiee : msi=$msi_size nsis=$nsis_size"

echo "▸ [7/8] commit + tag $TAG + push…"
git add Cargo.toml Cargo.lock package.json bun.lock \
        apps/inacord/package.json apps/nie-mcp/package.json apps/inacord/src-tauri/Cargo.toml \
        apps/inacord/src-tauri/Cargo.lock apps/inacord/src-tauri/tauri.conf.json \
        packages/nie/package.json packages/nie-bridge/package.json packages/nie-plugin/package.json
# Le bump peut avoir deja ete committe (relance apres un echec plus loin dans le pipeline) :
# un `git commit` sans rien a committer sort en erreur et, avec `set -e`, tue la release juste
# avant le tag. Le script doit etre rejouable, c'est sa raison d'etre.
# Tester l'arbre entier ne suffit pas : d'autres fichiers peuvent etre modifies sans qu'AUCUN
# des manifestes ci-dessus ne le soit (le bump ayant deja ete commite). C'est l'index qui compte.
if git diff --cached --quiet; then
	echo "  (versions deja committees — rien a commiter)"
else
	git commit -m "chore(release): bump $VERSION"
fi
git tag -a "$TAG" -m "niers $TAG"
git push origin main
git push origin "$TAG"

echo "▸ [8/8] GitHub Release $TAG (upload msi+nsis+sig+blender zip)…"
gh release create "$TAG" \
	--title "niers $TAG" \
	--notes "App desktop (Tauri v2) signée minisign + extension Blender v$BLENDER_VERSION. Détail : docs/PLAN.md, apps/inacord/ROADMAP.md." \
	"$MSI" "$MSI.sig" "$NSIS" "$NSIS.sig" "$BLENDER_ZIP"

echo "✓ Release $TAG publiée : https://github.com/aphrody-code/nie/releases/tag/$TAG"
echo "  → azalee.rosegriffon.fr/tools/niers + /latest.json se mettront à jour tout seuls (≤1h, cache dynamique)."

if [ "$SHIP_AZALEE" = "1" ]; then
	echo "▸ [bonus] --ship-azalee : redeploy azalee sur le VPS (scripts/redeploy-niers-tools.sh, dépôt rg)…"
	ssh ovh-vps-ubuntu-direct 'bash /home/ubuntu/rg/scripts/redeploy-niers-tools.sh' \
		|| ssh ovh-vps-ubuntu 'bash /home/ubuntu/rg/scripts/redeploy-niers-tools.sh'
fi
