#!/usr/bin/env pwsh
# packager-bases-explorer.ps1 — port PowerShell 7 de packager-bases-explorer.sh.
#
# Le .sh reste la référence sur le VPS Linux ; ce fichier est son équivalent pour le poste
# Windows, sans dépendance à Git Bash / MSYS (ni stat, ni gzip, ni du, ni find, ni wc).
#
# Prépare les bases QUI VOYAGENT AVEC l'installeur d'Inacord : le miroir du wiki
# (var/mirror.sqlite), la base de reverse (var/niers.sqlite) et le catalogue des épisodes
# (data/anime/episodes.db) sont compressés en .gz dans apps/inacord/src-tauri/resources/db/,
# lus par `bundle.resources`, puis décompressés vers %APPDATA%\dev.niers.explorer\db\ au
# premier lancement.
#
# Idempotent : une archive plus récente que sa source n'est pas recompressée.
#
# Usage :
#   pwsh -NoProfile -File scripts/packager-bases-explorer.ps1            # régénère ce qui a changé
#   pwsh -NoProfile -File scripts/packager-bases-explorer.ps1 --force    # recompresse tout

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# PowerShell 7.4+ transforme un code de retour natif non nul en exception quand EAP vaut Stop.
# On découple : chaque appel natif est testé explicitement sur $LASTEXITCODE, comme le .sh le
# fait avec `set -e` et ses `|| true` ciblés.
$PSNativeCommandUseErrorActionPreference = $false

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Cible = Join-Path $Root 'apps/inacord/src-tauri/resources/db'
$Force = ($args.Count -gt 0 -and $args[0] -eq '--force')

New-Item -ItemType Directory -Force -Path $Cible | Out-Null

function Write-Err([string] $Message) {
    [Console]::Error.WriteLine($Message)
}

# Équivalent de `stat -Lc%s` : suit le lien symbolique (var/mirror.sqlite en est un sur le VPS,
# bascule atomique du miroir) et renvoie la taille de la CIBLE, jamais celle du lien.
function Get-SourceSize([string] $Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.LinkType) {
        $target = $item.ResolveLinkTarget($true)
        if ($null -ne $target) {
            return ([System.IO.FileInfo] $target.FullName).Length
        }
    }
    return $item.Length
}

# Date de dernière modification, lien suivi (pour la comparaison `-nt` du .sh).
function Get-SourceMtime([string] $Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.LinkType) {
        $target = $item.ResolveLinkTarget($true)
        if ($null -ne $target) {
            return ([System.IO.FileInfo] $target.FullName).LastWriteTimeUtc
        }
    }
    return $item.LastWriteTimeUtc
}

# `gzip -6 -c "$source" > "$destination"` sans binaire externe : GZipStream de .NET.
# `Optimal` est le niveau maximal exposé par l'API ; l'archive n'est pas octet pour octet celle
# de `gzip -6`, mais c'est un flux gzip valide — seule chose que lise le Rust (`flate2`) au
# premier lancement de l'application.
function Compress-Gzip([string] $Source, [string] $Destination) {
    $partiel = "$Destination.part"
    $entree = [System.IO.File]::OpenRead($Source)
    try {
        $sortie = [System.IO.File]::Create($partiel)
        try {
            $gz = New-Object System.IO.Compression.GZipStream($sortie, [System.IO.Compression.CompressionLevel]::Optimal)
            try {
                $entree.CopyTo($gz)
            } finally {
                $gz.Dispose()
            }
        } finally {
            $sortie.Dispose()
        }
    } finally {
        $entree.Dispose()
    }
    Move-Item -LiteralPath $partiel -Destination $Destination -Force
}

# Format lisible façon `du -h` : base 1024, une décimale sous 10, entier au-delà.
function Format-Taille([long] $Octets) {
    $unites = @('', 'K', 'M', 'G', 'T')
    $valeur = [double] $Octets
    $i = 0
    while ($valeur -ge 1024 -and $i -lt ($unites.Count - 1)) {
        $valeur = $valeur / 1024
        $i++
    }
    if ($i -eq 0) { return "$Octets" }
    # Culture invariante : `du -h` écrit un point décimal, la locale FR écrirait une virgule.
    $inv = [System.Globalization.CultureInfo]::InvariantCulture
    if ($valeur -lt 10) {
        return ([string]::Format($inv, '{0:0.0}{1}', $valeur, $unites[$i]))
    }
    return ([string]::Format($inv, '{0:0}{1}', [math]::Ceiling($valeur), $unites[$i]))
}

# Compresse $Source vers $Cible/$Nom.gz si la source est plus récente que l'archive.
# Renvoie $true en cas de succès, $false sinon — l'appelant sort alors en 1, comme `set -e`
# le fait pour une fonction bash qui retourne 1.
function Invoke-Compression([string] $Source, [string] $Nom) {
    $archive = Join-Path $Cible "$Nom.gz"

    if ([string]::IsNullOrEmpty($Source) -or -not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        Write-Err "  ✗ $Nom : source absente ($Source)"
        return $false
    }

    $taille = Get-SourceSize $Source
    if ($taille -lt 1000000) {
        Write-Err "  ✗ $Nom : source suspecte ($taille octets) — non empaquetée"
        return $false
    }

    if ((-not $Force) -and (Test-Path -LiteralPath $archive -PathType Leaf)) {
        $mtimeArchive = (Get-Item -LiteralPath $archive -Force).LastWriteTimeUtc
        if ($mtimeArchive -gt (Get-SourceMtime $Source)) {
            $dejaLa = (Get-Item -LiteralPath $archive -Force).Length
            Write-Host "  = $Nom : à jour ($dejaLa octets compressés)"
            return $true
        }
    }

    Write-Host "  → $Nom : compression de $taille octets…"
    Compress-Gzip -Source $Source -Destination $archive
    $compresse = (Get-Item -LiteralPath $archive -Force).Length
    Write-Host "  ✓ $Nom : $compresse octets compressés"
    return $true
}

Write-Host "▸ bases embarquées d'Inacord → $Cible"

# Le miroir du wiki. Source canonique : var/mirror.sqlite (lien vers l'instantané courant, posé
# par scripts/donnees/miroir-inagle.sh) ; à défaut, le dernier instantané daté.
$Miroir = Join-Path $Root 'var/mirror.sqlite'
if (-not (Test-Path -LiteralPath $Miroir)) {
    $instantanes = @(Get-ChildItem -Path (Join-Path $Root 'var/miroir') -Filter 'inagle-*.sqlite' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending)
    $Miroir = if ($instantanes.Count -gt 0) { $instantanes[0].FullName } else { '' }
}
if (-not (Invoke-Compression $Miroir 'mirror.sqlite')) { exit 1 }

# La base de reverse. Contrairement au miroir, elle se reconstruit sur place (`niers rebuild`) :
# pas de lien, un seul fichier.
if (-not (Invoke-Compression (Join-Path $Root 'var/niers.sqlite') 'niers.sqlite')) { exit 1 }

# Le catalogue des épisodes de la série (packages/ietv → IETVCache), que la vue Cinéma présente
# à côté des cinématiques du jeu. ~290 Ko : le seuil de validité des deux autres (1 Mo) ne s'y
# applique pas — d'où le contrôle par le nombre d'épisodes plutôt que par la taille.
$Episodes = Join-Path $Root 'data/anime/episodes.db'
if (Test-Path -LiteralPath $Episodes -PathType Leaf) {
    $sortieSqlite = & sqlite3 $Episodes 'SELECT count(*) FROM episodes' 2>$null
    $codeSqlite = $LASTEXITCODE
    $nbEp = 0
    if ($codeSqlite -eq 0 -and $sortieSqlite) {
        $brut = ([string[]] $sortieSqlite)[0]
        $parse = 0
        if ([int]::TryParse(($brut -replace '\s', ''), [ref] $parse)) { $nbEp = $parse }
    }
    if ($nbEp -lt 100) {
        Write-Err "  ✗ episodes.db : $nbEp épisodes — base incomplète, non empaquetée"
        exit 1
    }
    Write-Host "  → episodes.db : $nbEp épisodes"
    Compress-Gzip -Source $Episodes -Destination (Join-Path $Cible 'episodes.db.gz')
    $tailleEp = (Get-Item -LiteralPath (Join-Path $Cible 'episodes.db.gz') -Force).Length
    Write-Host "  ✓ episodes.db : $tailleEp octets compressés"
} else {
    Write-Err "  ✗ episodes.db : source absente ($Episodes)"
    exit 1
}

# Contrôle final : `bundle.resources` porte le glob `resources/db/*.gz`. Un glob qui ne matche
# rien produirait un installeur SANS ses bases — exactement la release qu'on veut éviter, et que
# rien ne distinguerait ensuite d'une release complète.
$archives = @(Get-ChildItem -LiteralPath $Cible -Filter '*.gz' -File)
$nb = $archives.Count
if ($nb -lt 3) {
    Write-Err "ERREUR: $nb archive(s) sur 3 — l'installeur ne serait pas autonome."
    exit 1
}
$total = ($archives | Measure-Object -Property Length -Sum).Sum
Write-Host "  $nb bases prêtes à voyager avec l'installeur ($(Format-Taille $total) au total)"
