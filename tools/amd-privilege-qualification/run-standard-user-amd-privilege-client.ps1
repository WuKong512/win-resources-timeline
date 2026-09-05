#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = 'F76313FF123689C66A15112D43B1F87C33FE8DAD241AD6B98F0511247C3797A0'
$ConfigPath = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege\BROKER-CONFIG.json'

if (-not [Environment]::Is64BitProcess) {
    throw 'The standard-user client must run from x64 PowerShell.'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$integritySids = @($identity.Groups | ForEach-Object { $_.Value } | Where-Object { $_ -match '^S-1-16-' })
$integrityLevel = @($integritySids | ForEach-Object { [int64]($_ -replace '^S-1-16-', '') } | Measure-Object -Maximum).Maximum
if ($null -eq $integrityLevel -or $integrityLevel -ne 8192) {
    throw 'The client must be a normal non-elevated medium-integrity process.'
}
if (-not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
    throw "Broker config is missing: $ConfigPath"
}
if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
    throw "Exact release artifact is missing: $ArtifactPath"
}
$config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
if ([string]$config.installing_user_sid -ine $identity.User.Value) {
    throw 'The standard-user client SID does not match the installing-user SID authorized for this qualification pipe.'
}
$hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
if ($hash -ne $ExpectedArtifactSha256.ToUpperInvariant()) {
    throw "Release artifact SHA-256 mismatch. expected=$ExpectedArtifactSha256 actual=$hash"
}
& $ArtifactPath --client start --duration-ms 10000 --interval-ms 1000
exit $LASTEXITCODE
