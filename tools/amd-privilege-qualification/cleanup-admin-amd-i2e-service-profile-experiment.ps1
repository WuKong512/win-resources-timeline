#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'cleanup-state-contract.ps1')
. (Join-Path $PSScriptRoot 'i2e-service-profile-contract.ps1')

$ServiceName = $I2eServiceName
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$QualificationRoot = Join-Path $env:ProgramData $I2eOutputSubdirectory
$PointerPath = Join-Path $QualificationRoot 'I2E-EXPERIMENT-CURRENT.json'

function Assert-I2eCleanupAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'I2E cleanup requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
    }
    if (-not [Environment]::Is64BitProcess) { throw 'I2E cleanup requires x64 PowerShell.' }
}

function Read-I2eCleanupJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Write-I2eCleanupJson {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
}

function Invoke-I2eCleanupSc {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    $output = @(& $sc @Arguments 2>&1 | ForEach-Object { [string]$_ })
    if ($LASTEXITCODE -ne 0) {
        throw ('sc.exe {0} failed with exit code {1}: {2}' -f
            ($Arguments -join ' '), $LASTEXITCODE, ($output -join ' '))
    }
    $output
}

function Get-I2eCleanupServiceSnapshot {
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$ServiceName'" -ErrorAction Stop |
        Select-Object -First 1
    if ($null -eq $service) {
        return [pscustomobject]@{ present = $false; state = 'ABSENT'; process_id = 0L }
    }
    [pscustomobject]@{
        present = $true
        state = [string]$service.State
        process_id = [int64]$service.ProcessId
    }
}

function Stop-I2eCleanupService {
    $initial = Get-I2eCleanupServiceSnapshot
    if (-not $initial.present) {
        return [pscustomobject]@{ stop_exit_code = 1062; state = 'ABSENT'; process_id = 0L; disposition = 'SERVICE_ABSENT' }
    }
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    & $sc stop $ServiceName | Out-Null
    $exitCode = [int]$LASTEXITCODE
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $current = Get-I2eCleanupServiceSnapshot
        if (-not $current.present -or ($current.state -eq 'Stopped' -and $current.process_id -eq 0)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $current = Get-I2eCleanupServiceSnapshot
    $state = if ($current.present) { $current.state } else { 'ABSENT' }
    $serviceProcessId = if ($current.present) { [int64]$current.process_id } else { 0L }
    $disposition = Resolve-QualificationStopDisposition -StopExitCode $exitCode -ServiceState $state -ServiceProcessId $serviceProcessId -ServicePresent $current.present
    if ($disposition -eq 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0') {
        throw ('I2E cleanup failed closed; service state={0}, pid={1}, sc.exe exit={2}' -f $state, $serviceProcessId, $exitCode)
    }
    [pscustomobject]@{ stop_exit_code = $exitCode; state = $state; process_id = $serviceProcessId; disposition = $disposition }
}

function Remove-I2eCleanupService {
    $current = Get-I2eCleanupServiceSnapshot
    if (-not $current.present) { return $true }
    if ($current.state -ne 'Stopped' -or $current.process_id -ne 0) {
        throw 'Refusing to delete a running I2E service.'
    }
    Invoke-I2eCleanupSc -Arguments @('delete', $ServiceName) | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if (-not (Get-I2eCleanupServiceSnapshot).present) { return $true }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw ('I2E service registration remains after delete: {0}' -f $ServiceName)
}

function Test-I2eExactProcessGone {
    param([Parameter(Mandatory = $true)][int]$ProcessId, [Parameter(Mandatory = $true)][int64]$ProcessStartTime)
    try {
        $process = Get-Process -Id $ProcessId -ErrorAction Stop
        if ($ProcessStartTime -gt 0) {
            try { return ([int64]$process.StartTime.ToFileTimeUtc() -ne $ProcessStartTime) } catch { return $false }
        }
        return $false
    }
    catch { return $true }
}

function Get-I2eOwnedBrokerProcesses {
    $expected = [IO.Path]::GetFullPath($ArtifactPath)
    @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expected) } catch { $false }
    })
}

function Get-I2ePinnedAmdCliPath {
    param([Parameter(Mandatory = $true)][string]$ExperimentRoot)

    $preflightPath = Join-Path $ExperimentRoot 'AMD-CLI-PREFLIGHT.json'
    if (-not (Test-Path -LiteralPath $preflightPath -PathType Leaf)) {
        throw ('Pinned AMD CLI preflight is absent; refusing ownership inference: {0}' -f $preflightPath)
    }
    $preflight = Read-I2eCleanupJson -Path $preflightPath
    if (-not (Get-I2eCleanupPointerBoolean -Pointer $preflight -Name 'preflight_pass')) {
        throw ('Pinned AMD CLI preflight is not passing: {0}' -f $preflightPath)
    }
    $path = [string](Get-I2eCleanupPointerField -Pointer $preflight -Name 'path' -Default '')
    if ([string]::IsNullOrWhiteSpace($path)) {
        throw ('Pinned AMD CLI preflight has no path: {0}' -f $preflightPath)
    }
    try { return [IO.Path]::GetFullPath($path) }
    catch { throw ('Pinned AMD CLI preflight path is invalid: {0}' -f $path) }
}

function Get-I2eOwnedAmdProcesses {
    param([Parameter(Mandatory = $true)][string]$ExpectedAmdCliPath)
    $expected = [IO.Path]::GetFullPath($ExpectedAmdCliPath)
    @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expected) } catch { $false }
    })
}

$null = Assert-I2eCleanupAdministrator
if (-not (Test-Path -LiteralPath $PointerPath -PathType Leaf)) {
    throw ('I2E experiment pointer is absent; refusing cleanup without an owned experiment: {0}' -f $PointerPath)
}
$pointer = Read-I2eCleanupJson -Path $PointerPath
$experimentId = [string]$pointer.experiment_id
$serviceSid = [string](Get-I2eCleanupPointerField -Pointer $pointer -Name 'service_sid' -Default '')
# A pointer may survive service creation before Service SID resolution.
if ($experimentId -notmatch '^[0-9a-fA-F]{32}$') {
    throw 'I2E pointer has an invalid experiment identity.'
}
$failedAttemptClass = Resolve-I2eExperimentState -Pointer $pointer
if ($failedAttemptClass -eq 'CLOSED') {
    throw 'I2E experiment pointer is already finalized; refusing duplicate cleanup.'
}
$pairedGateConsumed = Test-I2ePairedGateConsumed -Pointer $pointer
$serviceCreateSucceeded = Get-I2eCleanupPointerBoolean -Pointer $pointer -Name 'service_create_succeeded'
$controlExecuted = Get-I2eCleanupPointerBoolean -Pointer $pointer -Name 'control_executed'
$treatmentExecuted = Get-I2eCleanupPointerBoolean -Pointer $pointer -Name 'treatment_executed'
$rightAddedByExperiment = Get-I2eCleanupPointerBoolean -Pointer $pointer -Name 'right_added_by_experiment'
$rightMutationState = [string](Get-I2eCleanupPointerField -Pointer $pointer -Name 'right_mutation_state' -Default 'NOT_STARTED')
if ($rightMutationState -eq 'STARTING' -and -not $rightAddedByExperiment) {
    throw 'I2E pointer records an unresolved right mutation attempt; refusing cleanup without authoritative mutation evidence.'
}
$experimentRoot = Join-Path $QualificationRoot $experimentId
if (-not (Test-Path -LiteralPath $experimentRoot -PathType Container)) {
    throw ('I2E experiment evidence root is absent; refusing pointer finalization: {0}' -f $experimentRoot)
}
$pinnedAmdCliPath = Get-I2ePinnedAmdCliPath -ExperimentRoot $experimentRoot
$policyRollbackState = Get-I2ePolicyRollbackState -Pointer $pointer
$policyRollbackStateSource = [string]$policyRollbackState.source
$rightNeedsRollback = Test-I2eExactRightRollbackRequired -Pointer $pointer
if ($rightAddedByExperiment -and $serviceSid -notmatch '^S-1-5-80-') {
    throw 'I2E pointer records an added right but has no valid Service SID identity.'
}
$attempt = '{0}-{1}' -f [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'), [Guid]::NewGuid().ToString('N')
$evidencePath = Join-Path $experimentRoot ('I2E-CLEANUP-RESULT-{0}.json' -f $attempt)
$finalPointerPath = Join-Path $experimentRoot ('I2E-EXPERIMENT-FINAL-{0}.json' -f $attempt)
$stopResult = $null
$rightRollback = $false
$policyRollbackVerified = $false
$effectiveTokenTeardownVerified = $false
$fullRollbackVerified = $false
$serviceStopAttempted = $false
$serviceStopVerified = $false
$serviceStateAfterStop = 'UNKNOWN'
$servicePidAfterStop = -1L
$ownedBrokerCountAfterStop = 0
$amdCliCountAfterStop = 0
$policyRemoveAttempted = $false
$policyReadbackReverified = $false
$policyRemoveSkippedReason = $null
$policyStateDrift = $false
$lsaRemoveAccountRightsCalls = 0
$direct = $null
$assigned = $null
$serviceRemoved = $false
$brokerGone = $false
$cliGone = $false
$cliSessions = @()
$currentPointerRemoved = $false

try {
    $serviceStopAttempted = $true
    $stopResult = Stop-I2eCleanupService
    $serviceStopVerified = $stopResult.state -eq 'Stopped' -and [int64]$stopResult.process_id -eq 0
    $serviceStateAfterStop = [string]$stopResult.state
    $servicePidAfterStop = [int64]$stopResult.process_id
    $ownedBrokerCountAfterStop = @(Get-I2eOwnedBrokerProcesses).Count
    $amdCliCountAfterStop = @(Get-I2eOwnedAmdProcesses -ExpectedAmdCliPath $pinnedAmdCliPath).Count
    if ($rightAddedByExperiment) {
        if ($rightNeedsRollback) {
            $policyRemoveAttempted = $true
            $lsaRemoveAccountRightsCalls = 1
            Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
        }
        else {
            $policyRemoveSkippedReason = 'ALREADY_VERIFIED_REMOVED'
        }
        $direct = Get-I2eDirectAccountRightsSnapshot -Label 'cleanup-after-rollback' -Sid $serviceSid
        $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
        $policyReadbackReverified = $true
        $policyReadbackAvailable = $direct.status -eq 'READ' -and $assigned.status -eq 'READ'
        $directRightPresent = $policyReadbackAvailable -and @($direct.direct_rights) -contains $I2eRequiredRight
        $assignmentPresent = $policyReadbackAvailable -and @($assigned.assigned_principals) -contains $serviceSid
        if (-not $policyReadbackAvailable -or $directRightPresent -or $assignmentPresent) {
            if (Test-I2ePolicyRollbackStateDrift -Pointer $pointer `
                    -ReadbackAvailable $policyReadbackAvailable `
                    -RightPresent $directRightPresent `
                    -AssignmentPresent $assignmentPresent) {
                $policyStateDrift = $true
                throw 'POLICY_ROLLBACK_STATE_DRIFT: pointer claimed the exact right was removed, but fresh dual LSA readback disagreed.'
            }
            throw 'I2E cleanup could not verify removal of the exact experiment right.'
        }
        $rightRollback = $true
        $policyRollbackVerified = $true
    }
    else {
        $rightRollback = $true
        $policyRollbackVerified = $true
    }

    # Persist policy rollback immediately.  A later service/process teardown or
    # deletion failure must not make a retry issue a duplicate LSA removal.
    $pointer.policy_rollback_verified = $policyRollbackVerified
    $pointer.effective_token_teardown_verified = $false
    $pointer.full_rollback_verified = $false
    $pointer.rollback_verified = $false
    $pointer.policy_remove_attempted = $policyRemoveAttempted
    $pointer.policy_readback_reverified = $policyReadbackReverified
    $pointer.policy_remove_skipped_reason = $policyRemoveSkippedReason
    $pointer.policy_rollback_state_source = $policyRollbackStateSource
    $pointer.policy_state_drift = $policyStateDrift
    $pointer.state = if ($policyRollbackVerified) { 'POLICY_ROLLBACK_COMPLETE' } else { 'POLICY_ROLLBACK_PENDING' }
    Write-I2eCleanupJson -Path $PointerPath -Value $pointer

    $brokerDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $brokers = @(Get-I2eOwnedBrokerProcesses)
        if ($brokers.Count -eq 0) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $brokerDeadline)
    $brokerGone = (@(Get-I2eOwnedBrokerProcesses).Count -eq 0)

    $roots = @(
        [string]$pointer.control_output_root,
        [string]$pointer.treatment_output_root
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) }
    foreach ($root in $roots) {
        foreach ($launchPath in @(Get-ChildItem -LiteralPath $root -Filter 'AMD-COUNTER-DISCOVERY-LAUNCH.json' -File -ErrorAction SilentlyContinue)) {
            $launch = Read-I2eCleanupJson -Path $launchPath.FullName
            $cliSessions += [pscustomobject]@{
                process_id = [int]$launch.target_pid
                process_start_time = [int64]$launch.target_process_start_time
            }
        }
    }
    $cliDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $cliGone = $true
        foreach ($session in $cliSessions) {
            if (-not (Test-I2eExactProcessGone -ProcessId $session.process_id -ProcessStartTime $session.process_start_time)) {
                $cliGone = $false
                break
            }
        }
        if ($cliGone) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $cliDeadline)
    if (-not $cliGone) { throw 'An owned AMD counter-discovery process identity remained; no unrelated process was killed.' }

    $serviceAfterStop = Get-I2eCleanupServiceSnapshot
    $serviceStateAfterStop = if ($serviceAfterStop.present) { [string]$serviceAfterStop.state } else { 'ABSENT' }
    $servicePidAfterStop = if ($serviceAfterStop.present) { [int64]$serviceAfterStop.process_id } else { 0L }
    $ownedBrokerCountAfterStop = @(Get-I2eOwnedBrokerProcesses).Count
    $amdCliCountAfterStop = @(Get-I2eOwnedAmdProcesses -ExpectedAmdCliPath $pinnedAmdCliPath).Count
    $rollbackVerification = Get-I2eRollbackVerification `
        -PolicyRollbackVerified $policyRollbackVerified `
        -ServicePresent $serviceAfterStop.present `
        -ServiceState $serviceStateAfterStop `
        -ServiceProcessId $servicePidAfterStop `
        -OwnedBrokerProcessCount $ownedBrokerCountAfterStop `
        -AmdCliProcessCount $amdCliCountAfterStop
    $serviceStopVerified = $rollbackVerification.service_stop_verified
    $effectiveTokenTeardownVerified = $rollbackVerification.effective_token_teardown_verified
    $fullRollbackVerified = $rollbackVerification.full_rollback_verified
    Write-I2eCleanupJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value ([ordered]@{
        schema = 'amd-service-profile-security-mutation-rollback/v1'
        qualification_only = $true
        experiment_id = $experimentId
        service_name = $ServiceName
        service_sid = $serviceSid
        right = $I2eRequiredRight
        right_added_by_experiment = $rightAddedByExperiment
        all_rights = $false
        service_stop_attempted = $serviceStopAttempted
        service_stop_verified = $serviceStopVerified
        service_state_after_stop = $serviceStateAfterStop
        service_pid_after_stop = $servicePidAfterStop
        owned_broker_process_count_after_stop = $ownedBrokerCountAfterStop
        amd_cli_process_count_after_stop = $amdCliCountAfterStop
        policy_remove_attempted = $policyRemoveAttempted
        lsa_remove_account_rights_calls = $lsaRemoveAccountRightsCalls
        policy_readback_reverified = $policyReadbackReverified
        policy_remove_skipped_reason = $policyRemoveSkippedReason
        policy_rollback_state_source = $policyRollbackStateSource
        policy_state_drift = $policyStateDrift
        policy_rollback_verified = $policyRollbackVerified
        effective_token_teardown_verified = $effectiveTokenTeardownVerified
        full_rollback_verified = $fullRollbackVerified
        rollback_verified = $fullRollbackVerified
        rollback_at_utc = [DateTime]::UtcNow.ToString('o')
        direct_verification = $direct
        assignment_verification = $assigned
        cleanup_invocation = $attempt
    })
    if (-not $fullRollbackVerified) {
        throw 'I2E cleanup cannot claim full rollback while policy or effective token teardown remains unverified.'
    }
    Remove-I2eCleanupService | Out-Null
    $serviceRemoved = -not (Get-I2eCleanupServiceSnapshot).present
    if (-not $serviceRemoved) { throw 'I2E service registration remained after verified rollback.' }
    $rollbackEvidence = Read-I2eCleanupJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json')
    $rollbackEvidence.service_registration_removed = $serviceRemoved
    Write-I2eCleanupJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value $rollbackEvidence

    $finalPointer = [ordered]@{}
    foreach ($property in $pointer.PSObject.Properties) {
        $finalPointer[$property.Name] = $property.Value
    }
    $finalPointer.schema = 'amd-service-profile-experiment-final/v1'
    $finalPointer.state = 'CLOSED'
    $finalPointer.service_create_succeeded = $serviceCreateSucceeded
    $finalPointer.control_executed = $controlExecuted
    $finalPointer.control_result = Get-I2eCleanupPointerField -Pointer $pointer -Name 'control_result'
    $finalPointer.right_added_by_experiment = $rightAddedByExperiment
    $finalPointer.treatment_executed = $treatmentExecuted
    $finalPointer.treatment_result = Get-I2eCleanupPointerField -Pointer $pointer -Name 'treatment_result'
    $finalPointer.policy_rollback_verified = $policyRollbackVerified
    $finalPointer.effective_token_teardown_verified = $effectiveTokenTeardownVerified
    $finalPointer.full_rollback_verified = $fullRollbackVerified
    $finalPointer.rollback_verified = $fullRollbackVerified
    $finalPointer.failed_attempt_class = $failedAttemptClass
    $finalPointer.paired_gate_consumed = $pairedGateConsumed
    $finalPointer.cleanup_attempt = $attempt
    $finalPointer.experiment_closed = $true
    $finalPointer.current_pointer_removed = $false
    $finalPointer.closed_at_utc = [DateTime]::UtcNow.ToString('o')
    Write-I2eCleanupJson -Path $finalPointerPath -Value $finalPointer

    Remove-Item -LiteralPath $PointerPath -Force -ErrorAction Stop
    $currentPointerRemoved = -not (Test-Path -LiteralPath $PointerPath -PathType Leaf)
    if (-not $currentPointerRemoved) {
        throw 'I2E CURRENT pointer remained after finalization; refusing successful cleanup.'
    }
    $finalPointer.current_pointer_removed = $true
    $finalPointer.finalization_verified_at_utc = [DateTime]::UtcNow.ToString('o')
    Write-I2eCleanupJson -Path $finalPointerPath -Value $finalPointer

    $result = [ordered]@{
        schema = 'amd-service-profile-cleanup/v1'
        qualification_only = $true
        experiment_id = $experimentId
        cleanup_attempt = $attempt
        cleanup_evidence_path = $evidencePath
        final_pointer_path = $finalPointerPath
        service_name = $ServiceName
        service_sid = $serviceSid
        failed_attempt_class = $failedAttemptClass
        paired_gate_consumed = $pairedGateConsumed
        service_create_succeeded = $serviceCreateSucceeded
        control_executed = $controlExecuted
        treatment_executed = $treatmentExecuted
        stop_control = $stopResult
        lsa_remove_account_rights_calls = $lsaRemoveAccountRightsCalls
        right_rollback_verified = $rightRollback
        policy_rollback_verified = $policyRollbackVerified
        effective_token_teardown_verified = $effectiveTokenTeardownVerified
        full_rollback_verified = $fullRollbackVerified
        service_stop_attempted = $serviceStopAttempted
        service_stop_verified = $serviceStopVerified
        service_state_after_stop = $serviceStateAfterStop
        service_pid_after_stop = $servicePidAfterStop
        owned_broker_process_count_after_stop = $ownedBrokerCountAfterStop
        amd_cli_process_count_after_stop = $amdCliCountAfterStop
        policy_remove_attempted = $policyRemoveAttempted
        policy_readback_reverified = $policyReadbackReverified
        policy_remove_skipped_reason = $policyRemoveSkippedReason
        policy_rollback_state_source = $policyRollbackStateSource
        policy_state_drift = $policyStateDrift
        service_registration_removed = $serviceRemoved
        broker_process_count_after_cleanup = @(Get-I2eOwnedBrokerProcesses).Count
        service_process_gone_after_cleanup = $brokerGone
        cli_session_processes_checked = $cliSessions.Count
        cli_process_gone_after_cleanup = $cliGone
        experiment_closed = $true
        current_pointer_finalized = $true
        current_pointer_removed = $currentPointerRemoved
        amd_installation_mutated = $false
        amd_registry_mutated = $false
        lsa_mutation_scope = 'exact Service SID + SeSystemProfilePrivilege only; no AllRights'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-I2eCleanupJson -Path $evidencePath -Value $result
    Write-Host ('I2E cleanup evidence retained at {0}' -f $evidencePath)
}
catch {
    $failure = [ordered]@{
        schema = 'amd-service-profile-cleanup/v1'
        qualification_only = $true
        experiment_id = $experimentId
        cleanup_attempt = $attempt
        cleanup_evidence_path = $evidencePath
        final_pointer_path = $finalPointerPath
        service_name = $ServiceName
        service_sid = $serviceSid
        failed_attempt_class = $failedAttemptClass
        paired_gate_consumed = $pairedGateConsumed
        service_create_succeeded = $serviceCreateSucceeded
        control_executed = $controlExecuted
        treatment_executed = $treatmentExecuted
        stop_control = $stopResult
        lsa_remove_account_rights_calls = $lsaRemoveAccountRightsCalls
        right_rollback_verified = $rightRollback
        policy_rollback_verified = $policyRollbackVerified
        effective_token_teardown_verified = $effectiveTokenTeardownVerified
        full_rollback_verified = $fullRollbackVerified
        service_stop_attempted = $serviceStopAttempted
        service_stop_verified = $serviceStopVerified
        service_state_after_stop = $serviceStateAfterStop
        service_pid_after_stop = $servicePidAfterStop
        owned_broker_process_count_after_stop = $ownedBrokerCountAfterStop
        amd_cli_process_count_after_stop = $amdCliCountAfterStop
        policy_remove_attempted = $policyRemoveAttempted
        policy_readback_reverified = $policyReadbackReverified
        policy_remove_skipped_reason = $policyRemoveSkippedReason
        policy_rollback_state_source = $policyRollbackStateSource
        policy_state_drift = $policyStateDrift
        service_registration_removed = $serviceRemoved
        service_process_gone_after_cleanup = $brokerGone
        cli_session_processes_checked = $cliSessions.Count
        cli_process_gone_after_cleanup = $cliGone
        experiment_closed = $false
        current_pointer_finalized = $false
        current_pointer_removed = $currentPointerRemoved
        error = $_.Exception.Message
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    try { Write-I2eCleanupJson -Path $evidencePath -Value $failure } catch { }
    throw
}
