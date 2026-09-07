#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedCleanup,
    [switch]$LibraryOnly,
    # Internal offline-test seam.  It is accepted only with the dedicated
    # test environment marker and always returns before machine mutation.
    [switch]$InternalTestOnlyPreMutationSentinel
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'i2e-runtime-library.ps1')
. (Join-Path $PSScriptRoot 'i2f-service-profile-contract.ps1')

$ServiceName = $I2fServiceName
$ServiceAccount = $I2fServiceAccount
$ServiceSidAccount = $I2fServiceSidAccount
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$QualificationRoot = Join-Path $env:ProgramData $I2fOutputSubdirectory
$ConfigPath = Join-Path $QualificationRoot 'I2F-CONFIG.json'

function Get-I2fCleanupRoots {
    @(Get-ChildItem -LiteralPath $QualificationRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^[0-9a-f]{32}$' })
}

function Get-I2fServiceSid {
    $sid = ([Security.Principal.NTAccount]::new($ServiceSidAccount)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    if ($sid -notmatch '^S-1-5-80-') { throw "Unexpected I2F Service SID: $sid" }
    $sid
}

if ($LibraryOnly) { return }
if ($InternalTestOnlyPreMutationSentinel -and $env:I2F_OFFLINE_TEST_SENTINEL -cne 'true') {
    throw 'The I2F cleanup pre-mutation sentinel is restricted to the offline test environment.'
}
if ($InternalTestOnlyPreMutationSentinel -and -not $ExecuteAuthorizedCleanup) {
    throw 'The I2F cleanup pre-mutation sentinel requires -ExecuteAuthorizedCleanup.'
}
$roots = Get-I2fCleanupRoots
if (-not $ExecuteAuthorizedCleanup) {
    [ordered]@{
        schema = 'amd-i2f-cleanup-plan/v2'
        qualification_only = $true
        service_name = $I2fServiceName
        right = $I2fRequiredRight
        all_rights = $false
        candidate_evidence_roots = @($roots | ForEach-Object { $_.FullName })
        fixed_cli_arguments = $I2fFixedArguments
        sampling = $false
        cleanup_order = @(
            'stop service',
            'verify Stopped/PID0 or absence',
            'verify pinned owned process absence',
            'pre-remove dual LSA readback',
            'remove exact right only when present',
            'verify dual LSA absence',
            'remove service registration',
            'verify full rollback'
        )
    } | ConvertTo-Json -Depth 20
    Write-Host 'I2F_CLEANUP_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, or AMD runtime was performed.'
    return
}
if ($InternalTestOnlyPreMutationSentinel) {
    Write-Host 'I2F_CLEANUP_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    Write-Host 'No service, LSA mutation, token adjustment, or AMD runtime was performed.'
    return
}

$null = Assert-I2eAdministrator

$root = $roots | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($null -eq $root) {
    throw "No I2F evidence root exists under $QualificationRoot."
}

$preflightPath = Join-Path $root.FullName 'I2F-AMD-CLI-PREFLIGHT.json'
if (-not (Test-Path -LiteralPath $preflightPath -PathType Leaf)) {
    throw 'I2F cleanup refuses to infer AMD CLI ownership without pinned preflight evidence.'
}
$preflight = Read-I2fJson -Path $preflightPath
$amdPath = [string](Get-I2ePropertyValue -Object $preflight.current_identity -Name 'path')
if ([string]::IsNullOrWhiteSpace($amdPath)) {
    throw 'I2F pinned AMD CLI preflight has no path.'
}

$configPath = Join-Path $root.FullName 'I2F-CONFIG.json'
$serviceSid = $null
if (Test-Path -LiteralPath $configPath -PathType Leaf) {
    $config = Read-I2fJson -Path $configPath
    $serviceSid = [string](Get-I2ePropertyValue -Object $config -Name 'service_sid')
}
$service = Get-I2eServiceSnapshot -ServiceName $ServiceName
if ([string]::IsNullOrWhiteSpace($serviceSid) -and $service.present) {
    $serviceSid = Get-I2fServiceSid
}

# I2F-LSA-AFTER-ADD is the immutable local evidence that the exact right was
# assigned by the experiment. Its absence means cleanup must not invent a
# policy mutation merely because this is the standalone cleanup entry point.
$rightAdded = Test-Path -LiteralPath (Join-Path $root.FullName 'I2F-LSA-AFTER-ADD.json') -PathType Leaf
$cleanupResult = Invoke-I2fCleanup -OutputRoot $root.FullName `
    -ServiceCreated ([bool]$service.present) -RightAdded $rightAdded `
    -ServiceName $ServiceName -BrokerArtifactPath $ArtifactPath `
    -ServiceSid $serviceSid -AmdCliPath $amdPath -PrimaryExperimentError $null

$state = $cleanupResult.state
Write-Host "I2F_POLICY_REMOVE_ATTEMPTED=$($state.policy_remove_attempted)"
Write-Host "I2F_LSA_REMOVE_CALLS=$($state.lsa_remove_account_rights_calls)"
Write-Host "I2F_EFFECTIVE_TOKEN_TEARDOWN_VERIFIED=$($state.effective_token_teardown_verified)"
Write-Host "I2F_FULL_ROLLBACK_VERIFIED=$($state.full_rollback_verified)"
if ($cleanupResult.evidence_write_error) {
    throw "I2F rollback evidence write failed: $($cleanupResult.evidence_write_error)"
}
if ($state.cleanup_required -and -not $state.full_rollback_verified) {
    throw 'I2F cleanup did not establish full rollback; evidence remains open for human recovery.'
}
