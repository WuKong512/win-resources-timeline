#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = '9FEB2BC942C74A6627BBC2716B450171C96A8E66617CE0624A3FC0FF69F3C464'
$ConfigPath = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege\BROKER-CONFIG.json'
. (Join-Path $PSScriptRoot 'token-integrity-contract.ps1')

if (-not [Environment]::Is64BitProcess) {
    throw 'The standard-user client must run from x64 PowerShell.'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$integrity = Get-CurrentProcessIntegrityLevel
if (-not (Test-QualificationClientIntegrity -IntegrityRid $integrity.integrity_rid)) {
    throw "The client must run at Medium integrity (RID 8192); actual integrity RID = $($integrity.integrity_rid)."
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
