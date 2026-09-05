#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = 'C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1'
$ConfigPath = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege\BROKER-CONFIG.json'
. (Join-Path $PSScriptRoot 'token-integrity-contract.ps1')

if (-not [Environment]::Is64BitProcess) {
    throw 'The standard-user counter-discovery client must run from x64 PowerShell.'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$integrity = Get-CurrentProcessIntegrityLevel
if (-not (Test-QualificationClientIntegrity -IntegrityRid $integrity.integrity_rid)) {
    throw "The counter-discovery client must run at Medium integrity (RID 8192); actual integrity RID = $($integrity.integrity_rid)."
}
if (-not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
    throw "Broker config is missing: $ConfigPath"
}
if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
    throw "Exact release artifact is missing: $ArtifactPath"
}
$config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
if ([string]$config.installing_user_sid -ine $identity.User.Value) {
    throw 'The standard-user counter-discovery SID does not match the installing-user SID authorized for this qualification pipe.'
}
$hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
if ($hash -ne $ExpectedArtifactSha256.ToUpperInvariant()) {
    throw "Release artifact SHA-256 mismatch. expected=$ExpectedArtifactSha256 actual=$hash"
}

# This is the only operation exposed by this handoff wrapper. It sends the typed semantic
# GetAmdCounterAvailability request; the broker supplies the executable, --list argv, working
# directory, timeout, job ownership, and evidence paths.
& $ArtifactPath --client counter-discovery
exit $LASTEXITCODE
