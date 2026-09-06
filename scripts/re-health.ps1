#!/usr/bin/env pwsh
# re-health.ps1 — santé de la stack RE niers. Lecture seule (n'écrit rien).
# Portage PowerShell 7 de scripts/re-health.sh, qui reste la référence sur le VPS Linux.
# Usage : pwsh -NoProfile -File scripts/re-health.ps1

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

Set-Location (Join-Path $PSScriptRoot '..')
$racine = (Get-Location).Path

$DB = if ($env:NIERS_DB) { $env:NIERS_DB } else { 'var/niers.sqlite' }
$BIN = 'target/release/niers'
# Sous Windows le binaire porte l'extension .exe : MSYS la résolvait toute seule pour `[ -x ]`,
# ici il faut la chercher explicitement. Ce n'est pas un assouplissement de la garde — la
# question posée reste « le binaire niers est-il construit ? ».
$BIN_EXE = if (Test-Path -LiteralPath $BIN -PathType Leaf) { $BIN }
elseif (Test-Path -LiteralPath "$BIN.exe" -PathType Leaf) { "$BIN.exe" }
else { $null }

# Racine du jeu : NIE_GAME_DIR (convention du reste du dépôt), NIERS_GAME_DIR (historique),
# sinon la racine du dépôt — sur une installation Steam, les deux coïncident.
$GAME_DIR = if ($env:NIE_GAME_DIR) { $env:NIE_GAME_DIR }
elseif ($env:NIERS_GAME_DIR) { $env:NIERS_GAME_DIR }
else { $racine }
$EXE = Join-Path $GAME_DIR 'nie_eacpatched.exe'

# Sans cela, PowerShell 7 RETIRE les séquences ANSI dès que la sortie est redirigée : la
# comparaison avec le .sh montrait des lignes sans couleur alors que le script en émet.
if ($PSStyle) { $PSStyle.OutputRendering = 'Ansi' }
# Les binaires du dépôt écrivent en UTF-8. Sans cela PowerShell décode leur sortie dans la page
# de codes OEM et « indexé » ressort en « index├® » : mesuré en comparant à la sortie du .sh.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

$ESC = [char]27
function ok([string]$m) { Write-Host ("  $ESC[32mOK$ESC[0m  $m") }
function ko([string]$m) { Write-Host ("  $ESC[31mKO$ESC[0m  $m") }
function hdr([string]$m) { Write-Host ''; Write-Host "=== $m ===" }

# Équivalent de `du -h` : taille lisible, suffixe K/M/G comme coreutils. Deux règles reprises
# de coreutils, et vérifiées contre `du -h` : il ARRONDIT AU SUPÉRIEUR, et il n'affiche une
# décimale qu'en dessous de 10. Formatage en culture INVARIANTE — sinon une locale française
# écrit « 64,1M » là où du écrit « 64.1M ».
function Get-TailleLisible([string]$p) {
    $o = [double](Get-Item -LiteralPath $p).Length
    $inv = [Globalization.CultureInfo]::InvariantCulture
    foreach ($u in @(@(1073741824, 'G'), @(1048576, 'M'), @(1024, 'K'))) {
        if ($o -ge $u[0]) {
            $v = $o / $u[0]
            if ($v -ge 10) { return ([Math]::Ceiling($v)).ToString($inv) + $u[1] }
            return ([Math]::Ceiling($v * 10) / 10).ToString('0.0', $inv) + $u[1]
        }
    }
    return ([int]$o).ToString($inv)
}

hdr 'Binaire & cible'
if ($BIN_EXE) { ok "niers build present ($BIN)" } else { ko 'niers ABSENT — cargo build --release -p nie-cli' }
if (Test-Path -LiteralPath $EXE -PathType Leaf) { ok "cible RE present ($(Get-TailleLisible $EXE))" } else { ko "exe RE ABSENT: $EXE" }
if (Test-Path -LiteralPath $DB -PathType Leaf) { ok "KB sqlite ($(Get-TailleLisible $DB))" } else { ko "KB ABSENTE: $DB — just re-seed" }

hdr 'Integrite KB (sqlite3)'
$sqlite = Get-Command sqlite3 -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if ($sqlite -and (Test-Path -LiteralPath $DB -PathType Leaf)) {
    foreach ($t in @('binary', 'function', 'xref', 'coverage', 'rtti_class', 'func_str_ref')) {
        # Le code de sortie du natif est lu IMMÉDIATEMENT, sans pipe intermédiaire : un
        # `| Select-Object -First 1` coupe la sortie du producteur et peut laisser
        # $LASTEXITCODE non défini — sous Set-StrictMode, le lire fait planter le script.
        $global:LASTEXITCODE = 0
        $brut = @(& $sqlite.Source $DB "SELECT COUNT(*) FROM $t" 2>$null)
        $rc = $LASTEXITCODE
        $n = if ($brut.Count -gt 0) { [string]$brut[0] } else { '' }
        if ($rc -eq 0 -and $n -match '^\d+$' -and ([int]$n) -gt 0) { ok "table $t = $n" } else { ko "table $t vide/absente" }
    }
    Write-Host '  --- snapshots couverture ---'
    $snap = & $sqlite.Source -header -column $DB `
        "SELECT ts, total_funcs, classified, printf('%.2f',pct) pct, named FROM coverage ORDER BY ts DESC LIMIT 5" 2>$null
    foreach ($l in @($snap)) { Write-Host ('  ' + $l) }
} else {
    ko "sqlite3 absent ou KB absente — saute l'integrite"
}

hdr 'Couverture (niers coverage)'
if ($BIN_EXE -and (Test-Path -LiteralPath $DB -PathType Leaf)) {
    $sortie = & $BIN_EXE coverage --db $DB 2>&1
    foreach ($l in @($sortie)) { Write-Host ('  ' + $l) }
} else {
    ko 'impossible (binaire ou KB manquant)'
}

hdr 'Dette de portage nie-engine (// EXTERN:)'
$total = 0
$fichiers = @(Get-ChildItem -Path 'crates/archive/nie-engine/src' -Filter '*.rs' -File -ErrorAction SilentlyContinue |
        Sort-Object Name)
foreach ($f in $fichiers) {
    # `grep -c` compte les LIGNES contenant le motif, pas les occurrences.
    $c = @(Select-String -LiteralPath $f.FullName -SimpleMatch -Pattern '// EXTERN:').Count
    $total += $c
    if ($c -gt 0) { Write-Host ('  {0,4}  {1}' -f $c, $f.Name) }
}
Write-Host '  ----'
Write-Host "  EXTERN_total=$total (fonctions C non encore portees en Rust)"

hdr 'Dette workspace (todo/unimplemented/dbg = deny, doit etre 0)'
# Fidèle à l'original : le motif est cherché sous `crates/*/src`, et les lignes contenant `//`
# sont écartées. NOTE : depuis la réorganisation en crates/{engine,forge,tools}/*/src, ce glob
# ne rencontre plus aucun fichier — le compte vaut 0 par construction, ici comme en bash.
$debt = 0
foreach ($d in @(Get-ChildItem -Path 'crates/*/src' -Directory -ErrorAction SilentlyContinue)) {
    $lignes = @(Select-String -Path (Join-Path $d.FullName '*') -Pattern 'todo!|unimplemented!|dbg!' -ErrorAction SilentlyContinue)
    $debt += @($lignes | Where-Object { $_.Line -notmatch '//' }).Count
}
if ($debt -eq 0) { ok '0 marqueur interdit' } else { ko "$debt marqueurs (clippy deny)" }

hdr 'Stores live'
$redis = Get-Command redis-cli -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
foreach ($db in @(0, 3)) {
    $up = $false
    if ($redis) {
        $global:LASTEXITCODE = 0
        & $redis.Source -u "redis://127.0.0.1/$db" ping *> $null
        $up = ($LASTEXITCODE -eq 0)
    }
    if ($up) { ok "redis db$db up" } else { ko "redis db$db DOWN" }
}
$svc = Get-Command systemctl -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
$actif = $false
if ($svc) {
    $global:LASTEXITCODE = 0
    & $svc.Source is-active --quiet nie-model-serve *> $null
    $actif = ($LASTEXITCODE -eq 0)
}
if ($actif) { ok 'nie-model-serve actif' } else { ko 'nie-model-serve inactif (502 possibles)' }

hdr 'Heartbeat RE'
$HB = 'var/re-heartbeat.log'
if (Test-Path -LiteralPath $HB -PathType Leaf) {
    # La DERNIERE ligne seule fait foi : sur `tail -2`, l'echec d'une passe reparee a la
    # suivante restait affiche en KO indefiniment.
    $last = Get-Content -LiteralPath $HB -Tail 1
    # 'No such file' = ancien mode demon (binaire absent) ; 'ERREUR heartbeat' = garde-fou
    # de scripts/re-heartbeat.sh (binaire, cible RE ou KB manquants).
    if ($last -match 'No such file|ERREUR heartbeat') { ko "heartbeat CASSE — $last" } else { ok "heartbeat: $last" }
    # Fraicheur : le cron est horaire ; au-dela de 3 h sans ligne, il ne tourne plus.
    $age = [int](((Get-Date) - (Get-Item -LiteralPath $HB).LastWriteTime).TotalMinutes)
    if ($age -le 180) { ok "heartbeat frais ($age min)" } else { ko "heartbeat fige depuis $age min — verifier 'crontab -l'" }
} else {
    ko "$HB absent"
}
Write-Host ''
exit 0
