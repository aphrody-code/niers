#!/usr/bin/env pwsh
# Publie dans ~/.local/bin tous les exécutables du dépôt — Rust et CLI Bun — et déduplique.
# Portage PowerShell 7 de scripts/installer-binaires.sh, qui reste la référence sur le VPS Linux.
#
# DÉDUPLICATION : des liens symboliques vers target/release, jamais des copies. 178 Mio de
# binaires (dont nie-editor à 82 Mio) ne sont donc écrits qu'une fois, et un `cargo build
# --release` met à jour la commande publiée sans réinstallation. Une copie, elle, se périme en
# silence — le pire défaut possible pour un dépôt qui mesure des octets.
#
# COLLISIONS : un lien n'est jamais posé par-dessus un exécutable étranger déjà dans le PATH.
#
# SOUS WINDOWS : New-Item -ItemType SymbolicLink échoue sans mode développeur ni privilège
# SeCreateSymbolicLinkPrivilege. On ne CROIT pas l'appel sur parole : on relit le LinkType
# après coup et on compte les copies séparément, exactement comme le `[ -L ]` du script bash.
#
# Usage : pwsh -NoProfile -File scripts/installer-binaires.ps1 [--dry-run]

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$racine = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$dest = if ($env:NIERS_BIN_DIR) { $env:NIERS_BIN_DIR } else { Join-Path $HOME '.local\bin' }
$sec = if ($args.Count -ge 1) { [string]$args[0] } else { '' }

if (-not (Test-Path -LiteralPath $dest)) { New-Item -ItemType Directory -Path $dest -Force | Out-Null }
$dest = (Resolve-Path -LiteralPath $dest).Path

$poses = 0
$sautes = 0
$refuses = 0
$copies = 0

# Chemin final réel : suit les liens symboliques, comme `readlink -f`. Rend $null si absent.
function Resolve-FinalPath([string]$p) {
    $item = Get-Item -LiteralPath $p -ErrorAction SilentlyContinue
    if (-not $item) { return $null }
    $cible = $item.ResolveLinkTarget($true)
    if ($cible) { return $cible.FullName }
    return $item.FullName
}

# Où la commande $nom se résout-elle déjà dans le PATH ? Équivalent de `command -v`.
function Get-CheminPath([string]$nom) {
    $c = Get-Command -Name $nom -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($c) { return $c.Source }
    return $null
}

# « Un exécutable ÉTRANGER déjà dans le PATH » : la comparaison bash portait sur le chemin exact
# ($dest/$nom), ce qui suffit sous Linux où le nom publié EST le nom du fichier. Sous Windows,
# PATHEXT résout `nie-catalog` vers `nie-catalog.cmd` : comparer les chemins exacts ferait
# refuser au script son propre lanceur. On compare donc le RÉPERTOIRE — même intention, même
# refus sur un binaire étranger, sans faux positif sur nos propres publications.
function Test-Etranger([string]$actuel) {
    if (-not $actuel) { return $false }
    return ((Split-Path -Path $actuel -Parent) -ne $dest)
}

function Show-Relatif([string]$p) {
    if ($p.StartsWith($racine + [IO.Path]::DirectorySeparatorChar)) {
        return $p.Substring($racine.Length + 1)
    }
    return $p
}

function New-Lien([string]$nom, [string]$cible) {
    if (-not (Test-Path -LiteralPath $cible)) {
        Write-Host ('  ??  {0,-20} cible absente ({1})' -f $nom, (Show-Relatif $cible))
        return
    }
    $publie = Join-Path $dest $nom
    $actuel = Get-CheminPath $nom
    if (Test-Etranger $actuel) {
        Write-Host ('  !!  {0,-20} REFUSÉ — {1} existe déjà dans le PATH' -f $nom, $actuel)
        $script:refuses++
        return
    }
    $dejaPose = Resolve-FinalPath $publie
    $cibleFinale = Resolve-FinalPath $cible
    if ($dejaPose -and $cibleFinale -and ($dejaPose -eq $cibleFinale)) {
        $script:sautes++
        return
    }
    if ($sec -ne '--dry-run') {
        try {
            New-Item -ItemType SymbolicLink -Path $publie -Target $cible -Force -ErrorAction Stop | Out-Null
        } catch {
            Write-Host ('  !!  {0,-20} ÉCHEC du lien symbolique' -f $nom)
            $script:refuses++
            return
        }
        # On ne CROIT pas l'appel sur parole : la contrainte de l'en-tête est « un lien, jamais
        # une copie », et c'est cette propriété-là qu'on mesure. Un succès ayant produit un
        # fichier réel est le faux vert que ce script existe pour éviter.
        $pose = Get-Item -LiteralPath $publie -ErrorAction SilentlyContinue
        if ((-not $pose) -or ($pose.LinkType -ne 'SymbolicLink')) {
            Write-Host ('  !!  {0,-20} COPIE et non lien — elle se périmera en silence' -f $nom)
            $script:copies++
            return
        }
    }
    Write-Host ('  ->  {0,-20} {1}' -f $nom, (Show-Relatif $cible))
    $script:poses++
}

Write-Host 'Binaires Rust (target/release) :'
$release = Join-Path $racine 'target\release'
if (Test-Path -LiteralPath $release) {
    # `[ -f ] && [ -x ]` côté bash : sous Windows l'exécutabilité est portée par l'extension.
    # On garde .exe/.bat/.cmd et les fichiers sans extension, on écarte .d/.so/.rlib comme
    # l'original, plus les artefacts MSVC que MSYS écartait déjà via -x (.pdb, .lib, .dll, .exp).
    $exclues = @('.d', '.so', '.rlib', '.pdb', '.lib', '.dll', '.exp', '.json', '.txt')
    # Tri ORDINAL : le glob bash trie par octets, Sort-Object selon la culture (il ignore la
    # ponctuation, ce qui déplace nie-bench.exe par rapport à nie_bench.exe).
    $fichiers = [string[]]@((Get-ChildItem -LiteralPath $release -File).Name)
    if ($fichiers.Count -gt 1) { [Array]::Sort($fichiers, [StringComparer]::Ordinal) }
    foreach ($nomFichier in $fichiers) {
        $ext = [IO.Path]::GetExtension($nomFichier).ToLowerInvariant()
        if ($exclues -contains $ext) { continue }
        if ($ext -ne '' -and $ext -ne '.exe' -and $ext -ne '.bat' -and $ext -ne '.cmd') { continue }
        New-Lien $nomFichier (Join-Path $release $nomFichier)
    }
}

# Les CLI Bun ne sont pas des exécutables : on publie un lanceur. `bun --bun` est obligatoire —
# `bun run` honore le shebang `#!/usr/bin/env node`, et node est proscrit ici.
Write-Host ''
Write-Host 'CLI Bun (lanceurs) :'
$specs = [ordered]@{
    'nie-catalog'  = 'packages/nie-catalog/src/cli.ts'
    'niers-azalee' = 'packages/azalee-tools/src/cli.ts'
    'niers-inagle' = 'packages/inagle/src/cli.ts'
    'niers-mcp'    = 'packages/mcp/src/cli.ts'
    'niers-bxc'    = 'apps/bxc/src/cli.ts'
}
foreach ($nom in $specs.Keys) {
    $src = $specs[$nom]
    if (-not (Test-Path -LiteralPath (Join-Path $racine $src))) {
        Write-Host ('  ??  {0,-20} source absente ({1})' -f $nom, $src)
        continue
    }
    # Le lanceur .cmd est l'équivalent Windows du shebang bash : PATHEXT le rend appelable
    # sous le nom publié, sans extension à taper.
    $publie = Join-Path $dest ($nom + '.cmd')
    $actuel = Get-CheminPath $nom
    if (Test-Etranger $actuel) {
        Write-Host ('  !!  {0,-20} REFUSÉ — {1} existe déjà' -f $nom, $actuel)
        $refuses++
        continue
    }
    # Le lanceur se place À LA RACINE du dépôt. Mesuré le 2026-09-02 : lancée depuis /tmp,
    # `nie-catalog etat` annonce « extrait : 0 tables » et « re : aucune mesure » — ses gisements
    # (var/mirror.sqlite, var/niers.sqlite) sont résolus relativement au cwd. Sans ce cd, une CLI
    # publiée globalement rapporte des gisements VIDES au lieu d'une erreur : un faux négatif.
    if ($sec -ne '--dry-run') {
        $contenu = "@echo off`r`ncd /d `"$racine`" || exit /b 1`r`nbun --bun `"$src`" %*`r`n"
        [IO.File]::WriteAllText($publie, $contenu, [Text.UTF8Encoding]::new($false))
    }
    Write-Host ('  ->  {0,-20} {1}' -f $nom, $src)
    $poses++
}

Write-Host ''
Write-Host ('{0} posés, {1} déjà à jour, {2} refusés (collision), {3} copies → {4}' -f `
        $poses, $sautes, $refuses, $copies, $dest)
if ($copies -gt 0) {
    Write-Host ''
    Write-Host "ATTENTION : $copies commande(s) publiée(s) en COPIE. Elles ne suivront pas un"
    Write-Host '`cargo build --release` et se périmeront sans le dire. Sous Windows, activez le mode'
    Write-Host 'développeur (ou accordez SeCreateSymbolicLinkPrivilege) et relancez.'
}
Write-Host ''
Write-Host 'Rappel de doctrine : `niers` est la seule CLI utilisateur. `nie-mem` et `nie-steam`'
Write-Host 'recouvrent `niers mem` et `niers steam` — publiés pour l''outillage, mais une commande'
Write-Host 'nouvelle s''écrit dans nie-cli, jamais dans un binaire de plus.'
exit 0
