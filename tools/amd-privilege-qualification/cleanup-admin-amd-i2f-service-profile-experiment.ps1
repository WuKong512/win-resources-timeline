#requires -Version 5.1
[CmdletBinding()]
param([switch]$ExecuteAuthorizedCleanup, [switch]$LibraryOnly)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'run-admin-amd-i2e-service-profile-experiment.ps1') -LibraryOnly
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

if ($LibraryOnly) { return }

$null = Assert-I2eAdministrator
$roots = Get-I2fCleanupRoots
if (-not $ExecuteAuthorizedCleanup) {
    [ordered]@{
        schema = 'amd-i2f-cleanup-plan/v1'
        qualification_only = $true
        service_name = $I2fServiceName
        right = $I2fRequiredRight
        all_rights = $false
        candidate_evidence_roots = @($roots.FullName)
        fixed_cli_arguments = $I2fFixedArguments
        sampling = $false
    } | ConvertTo-Json -Depth 20
    Write-Host 'I2F_CLEANUP_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, or AMD runtime was performed.'
    return
}

$service = Get-I2eServiceSnapshot
if ($service.present) {
    if ($service.state -ne 'Stopped' -or $service.process_id -ne 0) {
        $stopEvidence = Stop-I2eService
    } else {
        $stopEvidence = [pscustomobject]@{
            stop_exit_code = 0
            state = 'Stopped'
            process_id = 0L
            disposition = 'ALREADY_STOPPED_PID0'
        }
    }
} else {
    $stopEvidence = [pscustomobject]@{
        stop_exit_code = 1062
        state = 'ABSENT'
        process_id = 0L
        disposition = 'SERVICE_ABSENT'
    }
}

$root = $roots | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$serviceSid = $null
$rightState = $null
$policyRemoveAttempted = $false
$policyRollbackVerified = $false
$fullRollbackVerified = $false
if ($null -ne $root) {
    $configPath = Join-Path $root.FullName 'I2F-CONFIG.json'
    if (Test-Path -LiteralPath $configPath -PathType Leaf) {
        $config = Read-I2fJson -Path $configPath
        $serviceSid = [string]$config.service_sid
    }
}
if ([string]::IsNullOrWhiteSpace($serviceSid) -and $service.present) {
    $serviceSid = Get-I2fServiceSid
}
if (-not [string]::IsNullOrWhiteSpace($serviceSid)) {
    $rightState = Get-I2fRightState -ServiceSid $serviceSid
    if ($rightState.direct_present -or $rightState.assignment_present) {
        $policyRemoveAttempted = $true
        Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
    }
    $rightState = Get-I2fRightState -ServiceSid $serviceSid
    $policyRollbackVerified = -not $rightState.direct_present -and -not $rightState.assignment_present
}
$preflightPath = if ($null -ne $root) { Join-Path $root.FullName 'I2F-AMD-CLI-PREFLIGHT.json' } else { $null }
$amdPath = if ($null -ne $preflightPath -and (Test-Path -LiteralPath $preflightPath -PathType Leaf)) {
    [string](Read-I2fJson -Path $preflightPath).current_identity.path
} else {
    throw 'I2F cleanup refuses to infer AMD CLI ownership without pinned preflight evidence.'
}
$processEvidence = Assert-I2fNoOwnedProcesses -AmdCliPath $amdPath
$registrationRemoved = $false
if ((Get-I2eServiceSnapshot).present) {
    Remove-I2eService
}
$registrationRemoved = -not (Get-I2eServiceSnapshot).present
$effectiveTeardown = ($stopEvidence.state -eq 'Stopped' -and $stopEvidence.process_id -eq 0 -and
    $processEvidence.owned_broker_process_count -eq 0 -and $processEvidence.amd_cli_process_count -eq 0)
if ($null -ne $root -and $null -ne $rightState) {
    $fullRollbackVerified = Write-I2fRollbackEvidence -OutputRoot $root.FullName -RightAdded $true `
        -PolicyRollbackVerified $policyRollbackVerified `
        -EffectiveTokenTeardownVerified $effectiveTeardown `
        -ServiceRegistrationRemoved $registrationRemoved `
        -StopEvidence $stopEvidence -ProcessEvidence $processEvidence -RightState $rightState
    Write-Host "I2F_POLICY_REMOVE_ATTEMPTED=$policyRemoveAttempted"
    Write-Host "I2F_FULL_ROLLBACK_VERIFIED=$fullRollbackVerified"
}
if (-not $fullRollbackVerified) {
    throw 'I2F cleanup did not establish full rollback; evidence remains open for human recovery.'
}
