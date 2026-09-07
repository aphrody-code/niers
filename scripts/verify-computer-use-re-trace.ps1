[CmdletBinding()]
param(
    [string]$Binary = (Join-Path $PSScriptRoot '..\data\re\10-binary\input\nie.exe'),
    [string]$Database = (Join-Path $PSScriptRoot '..\var\niers.sqlite')
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $repo

function Invoke-Gate([string]$Package, [string]$Extra) {
    Write-Host "[gate] cargo test -p $Package $Extra"
    if ($Extra) { & cargo test -p $Package --tests --locked $Extra }
    else { & cargo test -p $Package --tests --locked }
    if ($LASTEXITCODE -ne 0) { throw "gate failed: $Package" }
}

if (-not (Test-Path -LiteralPath $Binary)) { throw "binary not found: $Binary" }
if (-not (Test-Path -LiteralPath $Database)) { throw "database not found: $Database" }
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Binary).Hash.ToLowerInvariant()
$size = (Get-Item -LiteralPath $Binary).Length
Write-Host "[binary] size=$size sha256=$hash"
if ($size -ne 33918464 -or $hash -ne 'b1fa04ea365868e5c8933aca393366f82d0d446187e2187f2737dc4fa2acd40c') { throw 'reference binary identity mismatch' }
$sqlite = Get-Command sqlite3 -ErrorAction SilentlyContinue
if (-not $sqlite) { throw 'sqlite3 is required to verify the canonical binary row' }
$row = (& $sqlite.Source -readonly $Database "SELECT id || '|' || base_addr || '|' || size FROM binary WHERE sha256='$hash';").Trim()
if ($row -ne '1|5368709120|33918464') { throw "canonical SQLite binary row mismatch: $row" }
Write-Host "[sqlite] binary_id=1 image_base=0x140000000 size=33918464"

Invoke-Gate 'nie-re' '--lib'
Invoke-Gate 'nie-trace' ''
Invoke-Gate 'nie-computer-use' 'session_binds_hash_binary_id_and_addresses'
Invoke-Gate 'nie-computer-use' 'session_rejects_wrong_build'
$sessionJson = & cargo run -q -p nie-computer-use --example verify_session -- $Binary $Database 1
if ($LASTEXITCODE -ne 0) { throw 'canonical ReSession verification failed' }
$session = $sessionJson | ConvertFrom-Json
if ($session.binary_id -ne 1 -or $session.size_bytes -ne $size -or $session.sha256 -ne $hash) { throw "invalid canonical session result: $sessionJson" }
Write-Host "[session] binary_id=$($session.binary_id) image_base=0x$([Convert]::ToString($session.image_base, 16))"
$json = & cargo run -q -p nie-cli -- computer-use nie-exe --executable $Binary
if ($LASTEXITCODE -ne 0) { throw 'computer-use executable probe failed' }
$probe = $json | ConvertFrom-Json
if (-not $probe.available -or $probe.surface -ne 'nie_exe') { throw "invalid probe result: $json" }
Write-Host "[probe] surface=$($probe.surface) available=$($probe.available)"
Write-Host '[result] reproducible RE/trace/computer-use gates passed'
