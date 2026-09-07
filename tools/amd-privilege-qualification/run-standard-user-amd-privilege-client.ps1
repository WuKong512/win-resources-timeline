#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedClient,
    [switch]$LibraryOnly,
    [switch]$InternalTestOnlyPreRuntimeSentinel
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'legacy-real-gate-contract.ps1')

$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = 'C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1'
$ConfigPath = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege\BROKER-CONFIG.json'

if ($LibraryOnly) {
    return
}

if ($InternalTestOnlyPreRuntimeSentinel) {
    if (-not $ExecuteAuthorizedClient -or $env:AMD_LEGACY_OFFLINE_TEST_SENTINEL -cne 'true') {
        throw 'The I2 power-client offline sentinel requires -ExecuteAuthorizedClient and AMD_LEGACY_OFFLINE_TEST_SENTINEL=true.'
    }
    Write-Host 'I2_POWER_CLIENT_AUTHORIZED_PRE_RUNTIME_SENTINEL=true'
    return
}

if (-not $ExecuteAuthorizedClient) {
    [ordered]@{
        qualification_only = $true
        historical_gate = 'I2_POWER_SAMPLING_CLIENT'
        real_gate_consumed = $I2LegacyRealGateConsumed
        real_execution_allowed = $I2PowerSamplingClientAllowed
        status = $I2LegacyStatus
        sampling = $true
        fixed_operation = 'start --duration-ms 10000 --interval-ms 1000'
    } | ConvertTo-Json -Depth 10
    Write-Host 'I2_POWER_CLIENT_PLAN_ONLY=true'
    Write-Host 'I2_LEGACY_RERUN=FORBIDDEN'
    Write-Host 'No broker connection, client process, or power sampling was performed.'
    return
}

if ($I2LegacyRealGateConsumed -or -not $I2PowerSamplingClientAllowed) {
    throw 'I2_POWER_CLIENT_RERUN_FORBIDDEN: the historical I2 power-sampling client gate is consumed. Use a fresh explicitly authorized experiment harness.'
}

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
