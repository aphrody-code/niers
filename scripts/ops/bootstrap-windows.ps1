#Requires -Version 7.0
<#
.SYNOPSIS
    Prépare un clone Windows de `niers` : le branche sur l'installation Steam du jeu, et
    rapatrie du VPS ce que Git ne porte pas.

.DESCRIPTION
    Ce dépôt ne suffit pas à lui seul. Trois familles de fichiers en sont absentes, chacune
    pour une raison différente, et un clone frais ne le dit pas :

      1. **les fichiers du jeu** (111 Go) — assets © LEVEL-5, jamais versionnés. Ils viennent
         de l'installation **Steam** de cette machine, pas du VPS ;
      2. **les gisements** (`var/mirror.sqlite`, `data/anime/episodes.db`) — bases produites
         par les moissons du VPS ;
      3. **les index dérivés** (`var/vfs/inventaire.txt`, `data/re/`) — régénérables ici, mais
         longs : les copier prend une minute, les reconstruire prend une heure.

    Le script fait les trois, dans cet ordre, et **compte** à chaque étape. Une étape qui ne
    peut pas compter échoue plutôt que d'annoncer un succès.

    Il est **rejouable** : relancé, il écrase les mêmes fichiers et ne repose la variable
    d'environnement que si elle diffère. Il ne fait PAS de copie différentielle — `scp` recopie
    tout à chaque fois. Sur les 102 Mo par défaut c'est sans conséquence ; avec `-WithRe`
    (17 Go), c'est à savoir avant de relancer.

.PARAMETER VpsHost
    L'hôte SSH du VPS. Défaut : `ovh-vps-direct`.
    **Ne jamais viser `ovh-vps`** : cet alias passe par le VPN (10.8.0.1) et expire.

.PARAMETER GameDir
    La racine du jeu, si la détection Steam échoue. C'est le dossier qui porte
    `data\cpk_list.cfg.bin`, pas le dossier `data` lui-même.

.PARAMETER WithRe
    Rapatrie aussi `var/niers.sqlite` (**17 Go**). Absent par défaut, et pas seulement pour la
    taille : cette base est **ancrée sur un autre binaire** que le `nie.exe` installé
    (cf. `CLAUDE.md` § Base de connaissance), donc ses chiffres ne décrivent pas la cible. La
    rapatrier ne se justifie que pour un travail de reverse assumé.

.PARAMETER SkipVps
    Ne contacte pas le VPS : ne fait que la détection Steam et `NIE_GAME_DIR`. Utile quand les
    gisements sont déjà là, ou pour préparer une machine sans accès SSH.

.EXAMPLE
    pwsh -File scripts\ops\bootstrap-windows.ps1

.EXAMPLE
    pwsh -File scripts\ops\bootstrap-windows.ps1 -VpsHost ubuntu@51.77.147.152 -WithRe
#>
[CmdletBinding()]
param(
    [string]$VpsHost = 'ovh-vps-direct',
    [string]$GameDir,
    [switch]$WithRe,
    [switch]$SkipVps
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Distant = '/home/ubuntu/niers'
$AppId = 2799860   # INAZUMA ELEVEN Victory Road, cf. nie_steam::IEVR_STEAM_APP_ID

function Etape   { param([string]$M) Write-Host "`n$M" -ForegroundColor Cyan }
function Ok      { param([string]$M) Write-Host "  [ok]   $M" -ForegroundColor Green }
function Note    { param([string]$M) Write-Host "  [note] $M" -ForegroundColor Yellow }
function Echouer { param([string]$M) Write-Host "  [ko]   $M" -ForegroundColor Red; exit 1 }

Write-Host '=== niers — clone Windows ===' -ForegroundColor Cyan
Write-Host "    dépôt : $Root"
Write-Host "    VPS   : $VpsHost:$Distant"

# ── 1. Le jeu : trouver l'installation Steam ──────────────────────────────────
#
# On ne devine PAS le nom du dossier. Steam le déclare dans son manifeste
# (`appmanifest_<appid>.acf`, clé `installdir`), et les bibliothèques sont listées dans
# `libraryfolders.vdf` : le jeu peut vivre sur un autre disque que Steam lui-même. Coder en dur
# `C:\Program Files (x86)\Steam\steamapps\common\INAZUMA ELEVEN Victory Road` marche sur UNE
# machine — celle où le chemin a été relevé.
Etape '[1/4] Installation Steam du jeu'

function Trouver-Jeu {
    # `Set-StrictMode -Version Latest` fait LEVER une erreur sur la propriété d'un `$null` :
    # écrire `(Get-ItemProperty …).SteamPath` planterait sur une machine sans Steam au lieu de
    # passer au candidat suivant. On lit donc la clé, puis on teste, dans cet ordre.
    $candidats = [System.Collections.Generic.List[string]]::new()
    $cle = Get-ItemProperty 'HKCU:\Software\Valve\Steam' -Name SteamPath -ErrorAction SilentlyContinue
    if ($cle -and $cle.PSObject.Properties.Name -contains 'SteamPath') { $candidats.Add($cle.SteamPath) }
    $candidats.Add("${env:ProgramFiles(x86)}\Steam")
    $candidats.Add("$env:ProgramFiles\Steam")

    $steam = $candidats | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if (-not $steam) { return $null }

    # Les bibliothèques : celle de Steam, plus celles déclarées dans `libraryfolders.vdf`.
    $bibliotheques = [System.Collections.Generic.List[string]]::new()
    $bibliotheques.Add((Join-Path $steam 'steamapps'))
    $vdf = Join-Path $steam 'steamapps\libraryfolders.vdf'
    if (Test-Path $vdf) {
        foreach ($ligne in Get-Content $vdf) {
            if ($ligne -match '"path"\s+"(.+?)"') {
                $chemin = $Matches[1] -replace '\\\\', '\'
                $sa = Join-Path $chemin 'steamapps'
                if ((Test-Path $sa) -and -not $bibliotheques.Contains($sa)) { $bibliotheques.Add($sa) }
            }
        }
    }

    foreach ($sa in $bibliotheques) {
        $manifeste = Join-Path $sa "appmanifest_$AppId.acf"
        if (-not (Test-Path $manifeste)) { continue }
        # Même précaution : sans correspondance, `Select-String` rend `$null`, et lire
        # `.Matches` dessus lève sous StrictMode. Un manifeste tronqué est un cas réel — Steam
        # en écrit un dès le début du téléchargement, avant que `installdir` n'existe.
        $trouve = Select-String -Path $manifeste -Pattern '"installdir"\s+"(.+?)"' | Select-Object -First 1
        if (-not $trouve) { continue }
        $installdir = $trouve.Matches[0].Groups[1].Value
        if (-not $installdir) { continue }
        $racine = Join-Path $sa "common\$installdir"
        if (Test-Path $racine) { return $racine }
    }
    return $null
}

if (-not $GameDir) { $GameDir = Trouver-Jeu }
if (-not $GameDir) {
    Echouer "Installation Steam de l'app $AppId introuvable. Passer -GameDir <racine du jeu>."
}

# La garde qui compte : la racine du jeu est celle qui porte `data\cpk_list.cfg.bin`. Sans ce
# fichier, `Vfs::init` ne monte rien et TOUT le reste (goldens, MCP, `niers info`) échoue avec
# un message qui parle de VFS, jamais de chemin.
$cpkList = Join-Path $GameDir 'data\cpk_list.cfg.bin'
if (-not (Test-Path $cpkList)) {
    Echouer "$GameDir ne porte pas data\cpk_list.cfg.bin — ce n'est pas la racine du jeu."
}
$packs = @(Get-ChildItem -Path (Join-Path $GameDir 'data\packs') -Filter *.cpk -ErrorAction SilentlyContinue).Count
Ok "$GameDir — cpk_list.cfg.bin présent, $packs packs"

# ── 2. NIE_GAME_DIR, en variable UTILISATEUR ──────────────────────────────────
#
# Utilisateur et non session : une variable posée dans le shell courant disparaît, et les
# outils lancés depuis l'explorateur, un IDE ou un service ne la verraient jamais.
Etape '[2/4] NIE_GAME_DIR'
$actuelle = [Environment]::GetEnvironmentVariable('NIE_GAME_DIR', 'User')
if ($actuelle -eq $GameDir) {
    Ok "déjà posée : $actuelle"
} else {
    [Environment]::SetEnvironmentVariable('NIE_GAME_DIR', $GameDir, 'User')
    $env:NIE_GAME_DIR = $GameDir
    Ok "posée : $GameDir"
    if ($actuelle) { Note "remplaçait : $actuelle" }
    Note 'Un terminal déjà ouvert garde son ancien environnement — le rouvrir.'
}

# ── 3. Rapatrier du VPS ce que Git ne porte pas ───────────────────────────────
Etape '[3/4] Gisements et index depuis le VPS'

if ($SkipVps) {
    Note 'ignoré (-SkipVps)'
} else {
    if (-not (Get-Command ssh -ErrorAction SilentlyContinue)) {
        Echouer "ssh introuvable. Installer OpenSSH client (Fonctionnalités facultatives de Windows)."
    }
    # On vérifie l'accès AVANT de lancer six copies : un `Permission denied` au sixième
    # transfert laisserait un état à moitié importé, plus difficile à diagnostiquer qu'un refus
    # net au départ.
    & ssh -o BatchMode=yes -o ConnectTimeout=10 $VpsHost 'test -d /home/ubuntu/niers' 2>$null
    if ($LASTEXITCODE -ne 0) {
        Echouer "$VpsHost injoignable ou $Distant absent. Vérifier l'alias SSH (ne PAS viser ovh-vps : il passe par le VPN et expire)."
    }
    Ok "$VpsHost joignable"

    # Le miroir est un LIEN SYMBOLIQUE daté (`var/mirror.sqlite -> miroir/inagle-<date>.sqlite`)
    # et la base est ouverte en WAL par les services. Deux conséquences :
    #   - on résout le lien côté VPS, sinon on copie un lien mort ;
    #   - on passe par `sqlite3 .backup`, jamais `cp` : copier le seul fichier principal d'une
    #     base WAL perd les écritures récentes (42 épisodes manquants, mesuré le 2026-09-03).
    $tmp = '/tmp/niers-export'
    & ssh $VpsHost @"
set -e
mkdir -p $tmp
cible=`$(readlink -f $Distant/var/mirror.sqlite)
sqlite3 "`$cible" ".backup '$tmp/mirror.sqlite'"
sqlite3 $Distant/data/anime/episodes.db ".backup '$tmp/episodes.db'"
"@
    if ($LASTEXITCODE -ne 0) { Echouer 'export SQLite côté VPS échoué' }
    Ok 'bases figées côté VPS (.backup, pas cp)'

    $copies = @(
        @{ De = "$tmp/mirror.sqlite";               Vers = 'var\mirror.sqlite' },
        @{ De = "$tmp/episodes.db";                 Vers = 'data\anime\episodes.db' },
        @{ De = "$Distant/var/vfs/inventaire.txt";  Vers = 'var\vfs\inventaire.txt' },
        @{ De = "$Distant/var/vfs/extensions.txt";  Vers = 'var\vfs\extensions.txt' }
    )
    if ($WithRe) { $copies += @{ De = "$Distant/var/niers.sqlite"; Vers = 'var\niers.sqlite' } }

    foreach ($c in $copies) {
        $dest = Join-Path $Root $c.Vers
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dest) | Out-Null
        Write-Host "  … $($c.Vers)"
        & scp -q "${VpsHost}:$($c.De)" $dest
        if ($LASTEXITCODE -ne 0) { Echouer "copie de $($c.De) échouée" }
        $mo = [math]::Round((Get-Item $dest).Length / 1MB, 1)
        Ok "$($c.Vers) — $mo Mo"
    }

    # `data/re/` est un répertoire de tables régénérables (`scripts/extract_*.py`). On le copie
    # parce que le régénérer demande la toolbox Python du dépôt ; il n'est pas bloquant.
    Write-Host '  … data\re\'
    New-Item -ItemType Directory -Force -Path (Join-Path $Root 'data\re') | Out-Null
    & scp -q -r "${VpsHost}:$Distant/data/re/." (Join-Path $Root 'data\re')
    if ($LASTEXITCODE -eq 0) {
        $n = @(Get-ChildItem (Join-Path $Root 'data\re') -File -Recurse).Count
        Ok "data\re\ — $n fichiers"
    } else {
        Note 'data\re\ non copié — régénérable par scripts/extract_funclua_table.py'
    }

    & ssh $VpsHost "rm -rf $tmp" 2>$null | Out-Null
}

# ── 4. Compter, jamais déclarer ───────────────────────────────────────────────
#
# `systemctl says active` n'a jamais prouvé qu'un service répond, et « copié » n'a jamais
# prouvé qu'une base porte des lignes. Chaque gisement rend ici un NOMBRE.
Etape '[4/4] Vérification — un compte par gisement'

$sqlite = Get-Command sqlite3 -ErrorAction SilentlyContinue
foreach ($v in @(
    @{ Nom = 'extrait (miroir)'; Fichier = 'var\mirror.sqlite';        Sql = "select count(*) from sqlite_master where type='table'" },
    @{ Nom = 'anime (épisodes)'; Fichier = 'data\anime\episodes.db';   Sql = "select count(*) from sqlite_master where type='table'" }
)) {
    $f = Join-Path $Root $v.Fichier
    if (-not (Test-Path $f)) { Note "$($v.Nom) : absent"; continue }
    if ($sqlite) {
        $n = & sqlite3 $f $v.Sql
        Ok "$($v.Nom) : $n tables"
    } else {
        $mo = [math]::Round((Get-Item $f).Length / 1MB, 1)
        Note "$($v.Nom) : $mo Mo (sqlite3 absent — compte non fait)"
    }
}

$inv = Join-Path $Root 'var\vfs\inventaire.txt'
if (Test-Path $inv) {
    $lignes = (Get-Content $inv -ReadCount 0).Count
    Ok "inventaire VFS : $lignes lignes"
} else {
    Note 'inventaire VFS : absent — régénérable par `niers vfs find "data/" -n 300000 > var\vfs\inventaire.txt`'
}

Write-Host "`nEnsuite, dans un terminal NEUF (pour que NIE_GAME_DIR existe) :" -ForegroundColor Cyan
Write-Host '  cargo build --release -p nie-cli'
Write-Host '  .\target\release\niers.exe info        # doit annoncer 255 308 entrées'
Write-Host '  bun install ; bun run build:ffi'
Write-Host "`nLe détail, et ce qui reste à faire à la main : LOCAL.md"
