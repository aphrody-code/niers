#!/usr/bin/env pwsh
# release-desktop.ps1 — port PowerShell 7 de release-desktop.sh (le .sh reste la référence sur
# le VPS Linux). Pipeline de release COMPLET pour l'app desktop Inacord (ex nie-explorer,
# identifiant Tauri conservé) :
#   bump versions → sync lockfiles → build signé (msi+nsis) → tag+push → GitHub Release
#   → (option) redeploy azalee.
#
# Usage :
#   $env:TAURI_SIGNING_PRIVATE_KEY_PATH = "$HOME/.tauri/niers.key"
#   pwsh -NoProfile -File scripts/release-desktop.ps1 0.5.0
#   pwsh -NoProfile -File scripts/release-desktop.ps1 0.5.0 --ship-azalee
#
# NOTE — le côté VPS n'a PAS besoin d'être redéployé à chaque release : `azalee.rosegriffon.fr/
# tools/niers` et `/tools/niers/latest.json` lisent la dernière release GitHub EN DIRECT
# (`apps/azalee/lib/niers-releases.ts`, revalidate=3600s) — ce script suffit à lui seul à publier
# une version que l'updater Tauri ET la page de download verront sous 1 h max, sans toucher au
# VPS. `--ship-azalee` ne sert que si le CODE d'azalee (pas niers) a aussi changé entre-temps.
#
# La clé de signature n'est JAMAIS lue ni affichée par ce script : seul son CHEMIN circule,
# passé à Tauri par la variable d'environnement TAURI_SIGNING_PRIVATE_KEY.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# PowerShell n'a ni `set -e` ni `pipefail` : chaque appel natif est testé sur $LASTEXITCODE.
$PSNativeCommandUseErrorActionPreference = $false

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $Root

function Write-Err([string] $Message) { [Console]::Error.WriteLine($Message) }

function Assert-Exit([string] $Etape) {
    if ($LASTEXITCODE -ne 0) {
        Write-Err "ERREUR: $Etape a échoué (code $LASTEXITCODE)."
        exit 1
    }
}

# Substitution regex dans un fichier, conservant l'encodage UTF-8 sans BOM.
# Remplace l'appel `sed -i` du .sh : Edit-like, jamais silencieux sur un fichier absent.
function Set-FileRegex([string] $Path, [string] $Pattern, [string] $Replacement, [switch] $Multiline) {
    $contenu = [System.IO.File]::ReadAllText($Path)
    $options = if ($Multiline) { [System.Text.RegularExpressions.RegexOptions]::Multiline }
               else { [System.Text.RegularExpressions.RegexOptions]::None }
    $nouveau = [regex]::Replace($contenu, $Pattern, $Replacement, $options)
    if ($nouveau -ne $contenu) {
        [System.IO.File]::WriteAllText($Path, $nouveau, (New-Object System.Text.UTF8Encoding($false)))
    }
}

$Version = if ($args.Count -gt 0) { $args[0] } else { '' }
$ShipAzalee = $false
foreach ($arg in $args) { if ($arg -eq '--ship-azalee') { $ShipAzalee = $true } }

if ([string]::IsNullOrEmpty($Version) -or $Version.StartsWith('--')) {
    Write-Err 'Usage: release-desktop.ps1 <version, ex: 0.5.0> [--ship-azalee]'
    exit 1
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    Write-Err "ERREUR: version attendue au format X.Y.Z (reçu: $Version)"
    exit 1
}
$Tag = "v$Version"

# ── 0. Garde-fous ────────────────────────────────────────────────────────────────────────
$statut = & git status --porcelain
Assert-Exit 'git status'
if ($statut) {
    Write-Err "ERREUR: arbre de travail non propre — commit/stash d'abord."
    exit 1
}

$branche = (& git rev-parse --abbrev-ref HEAD)
Assert-Exit 'git rev-parse HEAD'
if ("$branche".Trim() -ne 'main') {
    Write-Err 'ERREUR: doit être sur main (workflow main direct, cf. CLAUDE.md).'
    exit 1
}

& git rev-parse $Tag *> $null
if ($LASTEXITCODE -eq 0) {
    Write-Err "ERREUR: le tag $Tag existe déjà."
    exit 1
}

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Err 'ERREUR: gh CLI introuvable.'
    exit 1
}

$MaisonDefaut = if ($env:HOME) { $env:HOME } else { $env:USERPROFILE }
$KeyPath = if ($env:TAURI_SIGNING_PRIVATE_KEY_PATH) { $env:TAURI_SIGNING_PRIVATE_KEY_PATH }
           else { Join-Path $MaisonDefaut '.tauri/niers.key' }
if (-not (Test-Path -LiteralPath $KeyPath -PathType Leaf)) {
    Write-Err "ERREUR: clé de signature absente ($KeyPath)."
    Write-Err "  Génère-la une fois avec : bunx tauri signer generate -w $KeyPath --ci"
    Write-Err '  Puis colle la clé publique dans apps/inacord/src-tauri/tauri.conf.json (plugins.updater.pubkey).'
    exit 1
}

Write-Host "▸ [1/8] bump version → $Version (workspace Cargo + Bun)…"
Set-FileRegex 'Cargo.toml' '^version = "[0-9]*\.[0-9]*\.[0-9]*"' "version = `"$Version`"" -Multiline
Set-FileRegex 'package.json' '"version": "[0-9]*\.[0-9]*\.[0-9]*"' "`"version`": `"$Version`""
foreach ($f in @(
        'apps/inacord/package.json', 'apps/nie-mcp/package.json',
        'packages/nie/package.json', 'packages/nie-bridge/package.json',
        'packages/nie-plugin/package.json')) {
    if (Test-Path -LiteralPath $f -PathType Leaf) {
        Set-FileRegex $f '"version": "[0-9]*\.[0-9]*\.[0-9]*"' "`"version`": `"$Version`""
    }
}
Set-FileRegex 'apps/inacord/src-tauri/Cargo.toml' '^version = "[0-9]*\.[0-9]*\.[0-9]*"' "version = `"$Version`"" -Multiline
Set-FileRegex 'apps/inacord/src-tauri/tauri.conf.json' '"version": "[0-9]*\.[0-9]*\.[0-9]*"' "`"version`": `"$Version`""

Write-Host '▸ [2/8] sync lockfiles (Cargo.lock + bun.lock)…'
& cargo update --workspace --offline *> $null
if ($LASTEXITCODE -ne 0) {
    & cargo update --workspace
    Assert-Exit 'cargo update --workspace'
}
Push-Location 'apps/inacord/src-tauri'
try {
    & cargo update --workspace --offline *> $null
    if ($LASTEXITCODE -ne 0) {
        & cargo update --workspace
        Assert-Exit 'cargo update --workspace (src-tauri)'
    }
} finally { Pop-Location }
& bun install
Assert-Exit 'bun install'

Write-Host '▸ [3/8] sanity check (cargo check workspace + src-tauri)…'
& cargo check --workspace
Assert-Exit 'cargo check --workspace'
Push-Location 'apps/inacord/src-tauri'
try {
    & cargo check
    Assert-Exit 'cargo check (src-tauri)'
} finally { Pop-Location }

Write-Host '▸ [4/8] zip extension Blender (plugins/niers-blender, hors __pycache__)…'
$manifeste = Get-Content -LiteralPath 'plugins/niers-blender/blender_manifest.toml'
$ligneVersion = ($manifeste | Where-Object { $_ -match '^version' } | Select-Object -First 1)
$mVersion = [regex]::Match("$ligneVersion", '"([0-9.]+)"')
if (-not $mVersion.Success) {
    Write-Err "ERREUR: version de l'extension Blender illisible dans blender_manifest.toml."
    exit 1
}
$BlenderVersion = $mVersion.Groups[1].Value

$ZipStage = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Force -Path $ZipStage | Out-Null
# racine = nom de MODULE Python (le dossier source a un tiret). On copie le RÉPERTOIRE, pas
# `dir/*` : le glob de Copy-Item saute les entrées cachées, là où `cp -r dir/.` les emporte.
Copy-Item -LiteralPath 'plugins/niers-blender' -Destination (Join-Path $ZipStage 'niers') -Recurse -Force
Get-ChildItem -LiteralPath $ZipStage -Recurse -Directory -Filter '__pycache__' -ErrorAction SilentlyContinue |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }

$BlenderZip = Join-Path $Root "apps/inacord/src-tauri/target/release/bundle/niers-$BlenderVersion.zip"
# `zip` n'existe pas sur une install Windows standard (ni Git Bash, ni MSYS ne le fournissent) :
# Compress-Archive est ici natif, c'est déjà ce que le .sh appelait en repli.
Compress-Archive -Path (Join-Path $ZipStage 'niers') -DestinationPath $BlenderZip -Force
Remove-Item -LiteralPath $ZipStage -Recurse -Force
Write-Host "  → $BlenderZip (addon v$BlenderVersion)"

Write-Host '▸ [5/8] bases embarquées (miroir wiki + base RE → resources/db/*.gz)…'
# Ce que l'installeur emporte pour être utile SANS le jeu et SANS le dépôt. L'étape est ici,
# avant le build : `bundle.resources` est lu par le bundler, une archive écrite après coup
# n'entrerait dans aucun paquet. Le script s'arrête si une base manque — une release amputée de
# ses données s'installe et se signe exactement comme une release complète.
& pwsh -NoProfile -File (Join-Path $Root 'scripts/packager-bases-explorer.ps1')
Assert-Exit 'packager-bases-explorer.ps1'

Write-Host '▸ [6/8] build desktop signé (msi + nsis, minisign)…'
Push-Location 'apps/inacord'
# Le .sh exporte ces variables dans un sous-shell : elles meurent avec lui. PowerShell n'a pas
# de sous-shell, on restaure donc l'état précédent à la sortie.
$cleAvant = $env:TAURI_SIGNING_PRIVATE_KEY
try {
    # Tauri lit le CHEMIN de la clé ici, jamais son contenu : rien de secret ne transite par ce
    # script, et rien n'est affiché.
    $env:TAURI_SIGNING_PRIVATE_KEY = $KeyPath
    # `${VAR:-}` côté bash exporte une chaîne vide. Sous Windows, affecter '' SUPPRIME la
    # variable : on ne la touche donc que si elle est déjà définie, et Tauri traite une variable
    # absente comme un mot de passe vide.
    & bun run tauri build
    Assert-Exit 'bun run tauri build'
} finally {
    Pop-Location
    $env:TAURI_SIGNING_PRIVATE_KEY = $cleAvant
}

$Bundle = 'apps/inacord/src-tauri/target/release/bundle'
$Msi = Join-Path $Bundle "msi/Inacord_${Version}_x64_en-US.msi"
$Nsis = Join-Path $Bundle "nsis/Inacord_${Version}_x64-setup.exe"
foreach ($f in @($Msi, "$Msi.sig", $Nsis, "$Nsis.sig")) {
    if (-not (Test-Path -LiteralPath $f -PathType Leaf)) {
        Write-Err "ERREUR: artefact attendu absent: $f"
        exit 1
    }
}

# Un installeur peut exister, être signé, et ne PAS contenir l'application. C'est arrivé : le
# bundler empaquetait `export-bindings.exe` (182 Ko) à la place du binaire (30 Mo), avec une
# signature minisign parfaitement valide — rien dans la chaîne updater ne l'aurait refusé.
# Le seul contrôle qui l'attrape est la taille.
$MinMsiBytes = 5000000
$MinNsisBytes = 3000000
$msiSize = (Get-Item -LiteralPath $Msi -Force).Length
$nsisSize = (Get-Item -LiteralPath $Nsis -Force).Length
if ($msiSize -lt $MinMsiBytes) {
    Write-Err "ERREUR: le MSI ne fait que $msiSize octets (minimum $MinMsiBytes)."
    Write-Err "  L'application n'y est probablement pas — vérifier qu'un seul binaire est construit"
    Write-Err '  en release (cf. required-features de export-bindings dans src-tauri/Cargo.toml).'
    exit 1
}
if ($nsisSize -lt $MinNsisBytes) {
    Write-Err "ERREUR: l'installeur NSIS ne fait que $nsisSize octets (minimum $MinNsisBytes)."
    exit 1
}
Write-Host "  taille vérifiée : msi=$msiSize nsis=$nsisSize"

Write-Host "▸ [7/8] commit + tag $Tag + push…"
& git add Cargo.toml Cargo.lock package.json bun.lock `
    apps/inacord/package.json apps/nie-mcp/package.json apps/inacord/src-tauri/Cargo.toml `
    apps/inacord/src-tauri/Cargo.lock apps/inacord/src-tauri/tauri.conf.json `
    packages/nie/package.json packages/nie-bridge/package.json packages/nie-plugin/package.json
Assert-Exit 'git add'

# Le bump peut avoir déjà été committé (relance après un échec plus loin dans le pipeline) : un
# `git commit` sans rien à committer sort en erreur et tuerait la release juste avant le tag.
# Tester l'arbre entier ne suffit pas : d'autres fichiers peuvent être modifiés sans qu'AUCUN
# des manifestes ci-dessus ne le soit. C'est l'index qui compte.
& git diff --cached --quiet
if ($LASTEXITCODE -eq 0) {
    Write-Host '  (versions déjà committées — rien à commiter)'
} else {
    & git commit -m "chore(release): bump $Version"
    Assert-Exit 'git commit'
}
& git tag -a $Tag -m "niers $Tag"
Assert-Exit 'git tag'
& git push origin main
Assert-Exit 'git push origin main'
& git push origin $Tag
Assert-Exit "git push origin $Tag"

Write-Host "▸ [8/8] GitHub Release $Tag (upload msi+nsis+sig+blender zip)…"
& gh release create $Tag `
    --title "niers $Tag" `
    --notes "App desktop (Tauri v2) signée minisign + extension Blender v$BlenderVersion. Détail : docs/PLAN.md, apps/inacord/ROADMAP.md." `
    $Msi "$Msi.sig" $Nsis "$Nsis.sig" $BlenderZip
Assert-Exit 'gh release create'

Write-Host "✓ Release $Tag publiée : https://github.com/aphrody-code/nie/releases/tag/$Tag"
Write-Host '  → azalee.rosegriffon.fr/tools/niers + /latest.json se mettront à jour tout seuls (≤1h, cache dynamique).'

if ($ShipAzalee) {
    Write-Host '▸ [bonus] --ship-azalee : redeploy azalee sur le VPS (scripts/redeploy-niers-tools.sh, dépôt rg)…'
    & ssh ovh-vps-ubuntu-direct 'bash /home/ubuntu/rg/scripts/redeploy-niers-tools.sh'
    if ($LASTEXITCODE -ne 0) {
        & ssh ovh-vps-ubuntu 'bash /home/ubuntu/rg/scripts/redeploy-niers-tools.sh'
        Assert-Exit 'ssh redeploy-niers-tools.sh'
    }
}
