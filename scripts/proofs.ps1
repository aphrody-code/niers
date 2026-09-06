#!/usr/bin/env pwsh
# Rejoue les preuves uemu et rend un compte MESURÉ : N ✓ / N ✗ / N ⧗.
# Portage PowerShell 7 de scripts/proofs.sh, qui reste la référence sur le VPS Linux.
#
# Une preuve (`scripts/validate_*.py`) émule sous Unicorn la fonction RÉELLE de nie.exe et
# compare le portage bit à bit ; elle sort en 1 dès qu'une comparaison tombe. C'est l'oracle
# du dépôt : ce que Rust ne sait pas produire seul. Mais une preuve qui ne rejoue jamais dérive
# en silence — c'est le « golden muet = faux vert » que ce dépôt proscrit ailleurs.
#
# Usage :
#   pwsh -NoProfile -File scripts/proofs.ps1              # les 47
#   pwsh -NoProfile -File scripts/proofs.ps1 parabola     # celles dont le nom contient « parabola »
#   $env:PREUVES_TIMEOUT = 30; pwsh -NoProfile -File scripts/proofs.ps1
#
# PIÈGE : PowerShell n'a pas de pipefail. `uv run x.py | Select-Object` masque le code du
# producteur. On capture la sortie ET le code de sortie du processus lui-même, jamais du pipe.

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

Set-Location (Join-Path $PSScriptRoot '..')
# Les preuves écrivent en UTF-8 : sans cela le motif d'échec ressort en page OEM.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

$filtre = if ($args.Count -ge 1) { [string]$args[0] } else { '' }
$timeout_s = if ($env:PREUVES_TIMEOUT) { [int]$env:PREUVES_TIMEOUT } else { 90 }

$ok = 0
$ko = 0
$to = 0
$echecs = [System.Collections.Generic.List[string]]::new()

# Équivalent de `timeout <s> uv run <f>` : rend la sortie fusionnée (stdout+stderr) et le code,
# 124 en cas de dépassement, comme GNU timeout.
function Invoke-Preuve([string]$fichier, [int]$secondes) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = 'uv'
    $psi.ArgumentList.Add('run')
    $psi.ArgumentList.Add($fichier)
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.WorkingDirectory = (Get-Location).Path
    $p = [System.Diagnostics.Process]::Start($psi)
    $tOut = $p.StandardOutput.ReadToEndAsync()
    $tErr = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit($secondes * 1000)) {
        try { $p.Kill($true) } catch { }
        try { $p.WaitForExit(5000) | Out-Null } catch { }
        return @{ out = ''; rc = 124 }
    }
    $sortie = ($tOut.GetAwaiter().GetResult()) + ($tErr.GetAwaiter().GetResult())
    return @{ out = $sortie; rc = $p.ExitCode }
}

# Le glob bash trie par OCTETS (« . » < « _ », donc validate_intrusive_map avant
# validate_intrusive_map_c). Sort-Object trie selon la culture et ignore la ponctuation :
# l'ordre des 47 preuves en différait. Comparaison ordinale, pour rendre la même liste.
$preuves = [string[]]@((Get-ChildItem -Path 'scripts' -Filter "validate_*$filtre*.py" -File -ErrorAction SilentlyContinue).Name)
if ($preuves.Count -gt 1) { [Array]::Sort($preuves, [StringComparer]::Ordinal) }
if ($preuves.Count -eq 0) {
    Write-Host "aucune preuve ne correspond à « $filtre »"
    exit 1
}

foreach ($f in $preuves) {
    $nom = [IO.Path]::GetFileNameWithoutExtension($f)
    $r = Invoke-Preuve (Join-Path 'scripts' $f) $timeout_s
    $rc = $r.rc
    if ($rc -eq 0) {
        $ok++
        Write-Host ('  ✓ {0}' -f $nom)
    } elseif ($rc -eq 124) {
        $to++
        $echecs.Add("$nom — timeout ${timeout_s}s")
        Write-Host ('  ⧗ {0} — timeout {1}s' -f $nom, $timeout_s)
    } else {
        $ko++
        # Le motif dit POURQUOI : un UC_ERR_* est un problème d'oracle (mapping, instruction
        # non émulée), un écart de valeurs est un problème de portage. Ne pas les confondre.
        $m = [regex]::Match($r.out,
            'UC_ERR_[A-Z_]+|Invalid memory mapping|invalid instruction|ModuleNotFoundError|Traceback')
        $motif = if ($m.Success) { $m.Value } else { "exit=$rc" }
        $echecs.Add("$nom — $motif")
        Write-Host ('  ✗ {0} — {1}' -f $nom, $motif)
    }
}

$total = $ok + $ko + $to
Write-Host ''
Write-Host ('preuves uemu : {0} ✓ / {1} ✗ / {2} ⧗   (sur {3})' -f $ok, $ko, $to, $total)

if ($echecs.Count -gt 0) {
    Write-Host ''
    Write-Host 'à reprendre :'
    foreach ($e in $echecs) { Write-Host ('  {0}' -f $e) }
    Write-Host ''
    Write-Host "un UC_ERR_* accuse l'oracle (adresse ou mapping périmé, instruction non émulée par le TCG),"
    Write-Host 'un écart de valeurs accuse le portage. Ne jamais « corriger » la preuve pour la faire passer.'
}

if (($ko + $to) -eq 0) { exit 0 } else { exit 1 }
