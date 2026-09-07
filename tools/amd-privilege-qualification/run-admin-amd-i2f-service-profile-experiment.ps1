#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedExperiment,
    [switch]$LibraryOnly,
    # Internal offline-test seam.  It is accepted only with the dedicated
    # test environment marker and always returns before machine mutation.
    [switch]$InternalTestOnlyPreMutationSentinel
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Reuse only the side-effect-free runtime library.  The executable I2E wrapper
# is intentionally never dot-sourced here, so its parameter binder cannot
# overwrite I2F entrypoint state.
. (Join-Path $PSScriptRoot 'i2e-runtime-library.ps1')
. (Join-Path $PSScriptRoot 'i2f-service-profile-contract.ps1')

$ServiceName = $I2fServiceName
$ServiceAccount = $I2fServiceAccount
$ServiceSidAccount = $I2fServiceSidAccount
$ScServiceAccount = 'NT AUTHORITY\LocalService'
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = 'F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D'
$QualificationRoot = Join-Path $env:ProgramData $I2fOutputSubdirectory
$ConfigPath = Join-Path $QualificationRoot 'I2F-CONFIG.json'
$ControlPreflightPath = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-profile\3935ac9082954bcfb2b1f94c54cf95d7\AMD-CLI-PREFLIGHT.json'

function Write-I2fJson {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
}

function Read-I2fJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required I2F evidence is absent: $Path"
    }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Assert-I2fArtifact {
    if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
        throw "Missing I2F qualification artifact: $ArtifactPath"
    }
    $hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($hash -cne $ExpectedArtifactSha256.ToUpperInvariant()) {
        throw "I2F artifact hash mismatch; expected=$ExpectedArtifactSha256 actual=$hash"
    }
    if ((Get-I2ePeArchitecture -Path $ArtifactPath) -cne 'x64') {
        throw 'I2F qualification artifact must be x64.'
    }
    $hash
}

function Get-I2fControlAmdIdentity {
    $control = Read-I2fJson -Path $ControlPreflightPath
    if (-not [bool](Get-I2ePropertyValue -Object $control -Name 'preflight_pass')) {
        throw "Immutable I2E CONTROL AMD CLI preflight is not passing: $ControlPreflightPath"
    }
    $control
}

function Compare-I2fCurrentAmdIdentity {
    param([Parameter(Mandatory = $true)]$Control)
    $current = Get-I2eAmdCliPreflight
    $comparison = Compare-I2eAmdCliPreflight -Control $Control -Current $current
    [pscustomobject]@{
        control_preflight_path = $ControlPreflightPath
        control_identity = $Control
        current_identity = $current
        comparison = $comparison
        pass = [bool]$comparison.pass
    }
}

function Get-I2fServiceSid {
    $sid = ([Security.Principal.NTAccount]::new($ServiceSidAccount)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    if ($sid -notmatch '^S-1-5-80-') { throw "Unexpected I2F Service SID: $sid" }
    $sid
}

function Assert-I2fNoOwnedProcesses {
    param([Parameter(Mandatory = $true)][string]$AmdCliPath)
    $evidence = Get-I2fOwnedProcessEvidence -BrokerArtifactPath $ArtifactPath -AmdCliPath $AmdCliPath
    if ($evidence.owned_broker_process_count -ne 0 -or $evidence.amd_cli_process_count -ne 0) {
        throw "I2F owned-process gate failed; broker=$($evidence.owned_broker_process_count) amd_cli=$($evidence.amd_cli_process_count)"
    }
    $evidence
}

function Get-I2fRightState {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)
    $direct = Get-I2eDirectAccountRightsSnapshot -Label 'i2f-service-sid' -Sid $ServiceSid
    $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2fRequiredRight
    if ($direct.status -ne 'READ' -or $assigned.status -ne 'READ') {
        throw 'I2F LSA readback was unavailable; failing closed.'
    }
    [pscustomobject]@{
        direct = $direct
        assigned = $assigned
        direct_present = @($direct.direct_rights | Where-Object { $_ -ieq $I2fRequiredRight }).Count -gt 0
        assignment_present = @($assigned.assigned_principals | Where-Object { $_ -ieq $ServiceSid }).Count -gt 0
    }
}

function Assert-I2fRightAbsent {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)
    $state = Get-I2fRightState -ServiceSid $ServiceSid
    if ($state.direct_present -or $state.assignment_present) {
        throw 'I2F Service SID already has SeSystemProfilePrivilege; refusing pre-existing mutation.'
    }
    $state
}

function Add-I2fCleanupError {
    param([Parameter(Mandatory = $true)]$State, [Parameter(Mandatory = $true)][string]$Message)
    $existing = [string](Get-I2fPropertyValue -Object $State -Name 'cleanup_error' -Default '')
    $State.cleanup_error = if ([string]::IsNullOrWhiteSpace($existing)) {
        $Message
    } else {
        "{0}`n{1}" -f $existing, $Message
    }
}

function Invoke-I2fCleanup {
    param(
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][bool]$ServiceCreated,
        [Parameter(Mandatory = $true)][bool]$RightAdded,
        [Parameter(Mandatory = $true)][string]$ServiceName,
        [Parameter(Mandatory = $true)][string]$BrokerArtifactPath,
        [AllowNull()][string]$ServiceSid,
        [Parameter(Mandatory = $true)][string]$AmdCliPath,
        [AllowNull()][string]$PrimaryExperimentError
    )

    $state = New-I2fCleanupState
    $state.cleanup_required = $ServiceCreated -or $RightAdded
    $state.primary_experiment_error = $PrimaryExperimentError
    $rightState = $null

    if (-not $state.cleanup_required) {
        $state.cleanup_phase = 'NO_MUTATION_REQUIRED'
        $state.policy_rollback_verified = $true
        $state.policy_rollback_state_source = 'NO_RIGHT_ADDED'
        $state.service_registration_removed = $true
        $state.full_rollback_verified = $false
        $state.rollback_verified = $false
        $evidence = Write-I2fRollbackEvidence -OutputRoot $OutputRoot `
            -RightAddedByExperiment $RightAdded -State $state -RightState $rightState
        return [pscustomobject]@{
            state = $state
            right_state = $rightState
            evidence = $evidence
            evidence_write_error = if ($evidence.success) { $null } else { $evidence.error }
        }
    }

    $state.cleanup_phase = 'SERVICE_STOP'
    $serviceSnapshot = Get-I2fSafeServiceSnapshot -ServiceName $ServiceName
    if ($ServiceCreated) {
        $state.service_stop_attempted = $true
        try { $null = Stop-I2eService -ServiceName $ServiceName } catch { Add-I2fCleanupError -State $state -Message $_.Exception.Message }
        $serviceSnapshot = Get-I2fSafeServiceSnapshot -ServiceName $ServiceName
        if ($null -eq $serviceSnapshot.present) {
            $state.service_stop_verified = $false
            $state.service_state_after_stop = 'UNKNOWN'
            $state.service_pid_after_stop = $null
        } else {
            $state.service_state_after_stop = if ($serviceSnapshot.present) { [string]$serviceSnapshot.state } else { 'ABSENT' }
            $state.service_pid_after_stop = if ($serviceSnapshot.present) { [int64]$serviceSnapshot.process_id } else { 0L }
            $state.service_stop_verified = (-not $serviceSnapshot.present) -or
                ($serviceSnapshot.state -eq 'Stopped' -and $serviceSnapshot.process_id -eq 0)
        }
        if (-not $state.service_stop_verified) {
            if ([string]::IsNullOrWhiteSpace([string]$state.cleanup_error)) {
                Add-I2fCleanupError -State $state -Message 'Service stop was not verified as Stopped/PID0.'
            }
            $state.cleanup_phase = 'SERVICE_STOP'
            $evidence = Write-I2fRollbackEvidence -OutputRoot $OutputRoot `
                -RightAddedByExperiment $RightAdded -State $state -RightState $rightState
            return [pscustomobject]@{
                state = $state
                right_state = $rightState
                evidence = $evidence
                evidence_write_error = if ($evidence.success) { $null } else { $evidence.error }
            }
        }
    } else {
        $state.service_state_after_stop = if ($null -eq $serviceSnapshot.present) { 'UNKNOWN' } elseif ($serviceSnapshot.present) { [string]$serviceSnapshot.state } else { 'ABSENT' }
        $state.service_pid_after_stop = if ($null -eq $serviceSnapshot.present) { $null } elseif ($serviceSnapshot.present) { [int64]$serviceSnapshot.process_id } else { 0L }
    }

    $state.cleanup_phase = 'PROCESS_CHECK'
    $state.owned_process_check_attempted = $true
    try {
        $processEvidence = Get-I2fOwnedProcessEvidence -BrokerArtifactPath $BrokerArtifactPath -AmdCliPath $AmdCliPath
        $state.owned_broker_process_count_after_stop = [int64]$processEvidence.owned_broker_process_count
        $state.amd_cli_process_count_after_stop = [int64]$processEvidence.amd_cli_process_count
        $state.owned_process_check_verified = $true
    } catch {
        $state.owned_process_check_verified = $false
        $state.owned_broker_process_count_after_stop = $null
        $state.amd_cli_process_count_after_stop = $null
        Add-I2fCleanupError -State $state -Message $_.Exception.Message
    }
    $prePolicyDecision = Get-I2fCleanupDecision `
        -ServicePresent ([bool]$serviceSnapshot.present) `
        -ServiceState ([string]$state.service_state_after_stop) `
        -ServiceProcessId $state.service_pid_after_stop `
        -StopAttempted $state.service_stop_attempted `
        -StopVerified $state.service_stop_verified `
        -ProcessCheckAttempted $state.owned_process_check_attempted `
        -ProcessCheckVerified $state.owned_process_check_verified `
        -BrokerCount $state.owned_broker_process_count_after_stop `
        -AmdCliCount $state.amd_cli_process_count_after_stop `
        -RightDirectPresent $false `
        -RightAssignmentPresent $false `
        -RightReadbackVerified $false `
        -PolicyRollbackVerified $false `
        -ServiceRegistrationPresent ([bool]$serviceSnapshot.present) `
        -PolicyRollbackPreviouslyVerified $false
    $state.effective_token_teardown_verified = $prePolicyDecision.effective_token_teardown_verified
    if (-not $state.effective_token_teardown_verified) {
        if ($state.owned_process_check_verified -and
            (($state.owned_broker_process_count_after_stop -gt 0) -or
                ($state.amd_cli_process_count_after_stop -gt 0))) {
            Add-I2fCleanupError -State $state -Message 'Owned qualification or AMD CLI process remains; policy rollback is forbidden.'
        } elseif ([string]::IsNullOrWhiteSpace([string]$state.cleanup_error)) {
            Add-I2fCleanupError -State $state -Message 'Effective token teardown could not be verified.'
        }
        $state.cleanup_phase = 'PROCESS_CHECK'
        $evidence = Write-I2fRollbackEvidence -OutputRoot $OutputRoot `
            -RightAddedByExperiment $RightAdded -State $state -RightState $rightState
        return [pscustomobject]@{
            state = $state
            right_state = $rightState
            evidence = $evidence
            evidence_write_error = if ($evidence.success) { $null } else { $evidence.error }
        }
    }

    $state.cleanup_phase = 'POLICY_READBACK'
    if (-not $RightAdded) {
        $state.policy_rollback_verified = $true
        $state.policy_rollback_state_source = 'NO_RIGHT_ADDED'
        $state.policy_remove_skipped_reason = 'NO_RIGHT_ADDED'
    } elseif ([string]::IsNullOrWhiteSpace($ServiceSid)) {
        Add-I2fCleanupError -State $state -Message 'Exact Service SID is unavailable; policy rollback is forbidden.'
    } else {
        try {
            $rightState = Get-I2fRightState -ServiceSid $ServiceSid
            $state.direct_verification = $rightState.direct
            $state.assignment_verification = $rightState.assigned
            $state.policy_pre_remove_readback_verified = $true
            $policyDecision = Get-I2fCleanupDecision `
                -ServicePresent ([bool]$serviceSnapshot.present) `
                -ServiceState ([string]$state.service_state_after_stop) `
                -ServiceProcessId $state.service_pid_after_stop `
                -StopAttempted $state.service_stop_attempted `
                -StopVerified $state.service_stop_verified `
                -ProcessCheckAttempted $state.owned_process_check_attempted `
                -ProcessCheckVerified $state.owned_process_check_verified `
                -BrokerCount $state.owned_broker_process_count_after_stop `
                -AmdCliCount $state.amd_cli_process_count_after_stop `
                -RightDirectPresent ([bool]$rightState.direct_present) `
                -RightAssignmentPresent ([bool]$rightState.assignment_present) `
                -RightReadbackVerified $true `
                -PolicyRollbackVerified $false `
                -ServiceRegistrationPresent ([bool]$serviceSnapshot.present) `
                -PolicyRollbackPreviouslyVerified $false
            if (-not $policyDecision.policy_remove_required) {
                $state.policy_remove_skipped_reason = 'ALREADY_ABSENT'
                $state.policy_rollback_state_source = 'FRESH_DUAL_READBACK'
                $state.policy_rollback_verified = $true
            } else {
                $state.policy_remove_attempted = $true
                $state.lsa_remove_account_rights_calls = [int64]$state.lsa_remove_account_rights_calls + 1
                try {
                    Remove-I2eExactServiceProfileRight -ServiceSid $ServiceSid
                    $state.policy_right_removed = $true
                } catch {
                    Add-I2fCleanupError -State $state -Message $_.Exception.Message
                }
                if ($state.policy_remove_attempted -and $state.policy_right_removed) {
                    try {
                        $rightState = Get-I2fRightState -ServiceSid $ServiceSid
                        $state.direct_verification = $rightState.direct
                        $state.assignment_verification = $rightState.assigned
                        $state.policy_rollback_state_source = 'POST_REMOVE_DUAL_READBACK'
                        $state.policy_rollback_verified = -not $rightState.direct_present -and
                            -not $rightState.assignment_present
                        if (-not $state.policy_rollback_verified) {
                            Add-I2fCleanupError -State $state -Message 'Exact right remained after removal readback.'
                        }
                    } catch {
                        Add-I2fCleanupError -State $state -Message $_.Exception.Message
                    }
                }
            }
        } catch {
            $state.policy_pre_remove_readback_verified = $false
            $state.policy_readback_error = $_.Exception.Message
            Add-I2fCleanupError -State $state -Message $_.Exception.Message
        }
    }
    if (-not $state.policy_rollback_verified) {
        $state.cleanup_phase = 'POLICY_ROLLBACK'
        $evidence = Write-I2fRollbackEvidence -OutputRoot $OutputRoot `
            -RightAddedByExperiment $RightAdded -State $state -RightState $rightState
        return [pscustomobject]@{
            state = $state
            right_state = $rightState
            evidence = $evidence
            evidence_write_error = if ($evidence.success) { $null } else { $evidence.error }
        }
    }

    $state.cleanup_phase = 'SERVICE_REGISTRATION'
    if ($ServiceCreated) {
        $state.service_registration_remove_attempted = $true
        try {
            Remove-I2eService -ServiceName $ServiceName
            $afterDelete = Get-I2fSafeServiceSnapshot -ServiceName $ServiceName
            $state.service_registration_removed = $null -ne $afterDelete.present -and
                -not $afterDelete.present
            if (-not $state.service_registration_removed) {
                Add-I2fCleanupError -State $state -Message 'I2F service registration remained after delete.'
            }
        } catch {
            Add-I2fCleanupError -State $state -Message $_.Exception.Message
            $state.service_registration_removed = $false
        }
    } else {
        $state.service_registration_removed = $true
    }
    $state.full_rollback_verified = $state.service_stop_verified -and
        $state.owned_process_check_verified -and
        $state.owned_broker_process_count_after_stop -eq 0 -and
        $state.amd_cli_process_count_after_stop -eq 0 -and
        $state.effective_token_teardown_verified -and
        $state.policy_rollback_verified -and
        $state.service_registration_removed
    $state.rollback_verified = $state.full_rollback_verified
    $state.cleanup_phase = if ($state.full_rollback_verified) { 'COMPLETE' } else { 'SERVICE_REGISTRATION' }
    $evidence = Write-I2fRollbackEvidence -OutputRoot $OutputRoot `
        -RightAddedByExperiment $RightAdded -State $state -RightState $rightState
    [pscustomobject]@{
        state = $state
        right_state = $rightState
        evidence = $evidence
        evidence_write_error = if ($evidence.success) { $null } else { $evidence.error }
    }
}

function Wait-I2fServiceEvidence {
    param([Parameter(Mandatory = $true)][string]$OutputRoot)
    $resultPath = Join-Path $OutputRoot 'I2F-COUNTER-DISCOVERY-RESULT.json'
    $errorPath = Join-Path $OutputRoot 'I2F-SERVICE-HARNESS-ERROR.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    do {
        if ((Test-Path -LiteralPath $resultPath -PathType Leaf) -or
            (Test-Path -LiteralPath $errorPath -PathType Leaf)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    if (Test-Path -LiteralPath $errorPath -PathType Leaf) {
        throw "I2F service harness failed: $(Get-Content -LiteralPath $errorPath -Raw)"
    }
    if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
        throw 'I2F did not produce a bounded counter-discovery result.'
    }
    Read-I2fJson -Path $resultPath
}

if ($LibraryOnly) { return }
if ($InternalTestOnlyPreMutationSentinel -and $env:I2F_OFFLINE_TEST_SENTINEL -cne 'true') {
    throw 'The I2F pre-mutation sentinel is restricted to the offline test environment.'
}
if ($InternalTestOnlyPreMutationSentinel -and -not $ExecuteAuthorizedExperiment) {
    throw 'The I2F pre-mutation sentinel requires -ExecuteAuthorizedExperiment.'
}
if (-not $ExecuteAuthorizedExperiment) {
    Get-I2fExperimentPlan -ArtifactSha256 $ExpectedArtifactSha256 | ConvertTo-Json -Depth 20
    Write-Host 'I2F_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, token adjustment, or AMD runtime was performed.'
    return
}
if ($InternalTestOnlyPreMutationSentinel) {
    Write-Host 'I2F_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    Write-Host 'No service, LSA mutation, token adjustment, or AMD runtime was performed.'
    return
}

$null = Assert-I2eAdministrator

$artifactHash = Assert-I2fArtifact
if ((Get-I2eServiceSnapshot -ServiceName $ServiceName).present) { throw "I2F service already exists: $ServiceName" }
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) { throw "Stale I2F config exists: $ConfigPath" }
$controlIdentity = Get-I2fControlAmdIdentity
$identityCheck = Compare-I2fCurrentAmdIdentity -Control $controlIdentity
$scope = [Guid]::NewGuid().ToString('N')
$outputRoot = Join-Path $QualificationRoot $scope
New-Item -ItemType Directory -Force -Path $QualificationRoot, $outputRoot | Out-Null
Write-I2fJson -Path (Join-Path $outputRoot 'I2F-AMD-CLI-PREFLIGHT.json') -Value ([ordered]@{
        schema = 'amd-i2f-amd-cli-preflight-comparison/v1'
        qualification_only = $true
        control_preflight_path = $ControlPreflightPath
        experiment_id = $scope
        control_identity = $identityCheck.control_identity
        current_identity = $identityCheck.current_identity
        comparison = $identityCheck.comparison
        pass = $identityCheck.pass
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
if (-not $identityCheck.pass) {
    throw "I2F AMD CLI identity drift detected: $($identityCheck.comparison.differing_fields -join ', ')"
}

$serviceCreated = $false
$rightAdded = $false
$serviceSid = $null
$amdCliPath = [string]$identityCheck.current_identity.path
$result = $null
$cleanupResult = $null
$primaryExperimentError = $null

try {
    try {
        $binPath = '"{0}" --service-profile-enable-counter-service' -f $ArtifactPath
        $createArgs = New-QualificationServiceCreateArguments -ServiceName $ServiceName -BinPath $binPath `
            -ServiceAccount $ScServiceAccount -DisplayName 'Resource Timeline AMD I2F self-enable qualification'
        Invoke-I2eSc -Arguments $createArgs | Out-Null
        $serviceCreated = $true
        Invoke-I2eSc -Arguments @('sidtype', $ServiceName, 'unrestricted') | Out-Null
        Assert-I2eServiceSidType -ServiceName $ServiceName
        $serviceSid = Get-I2fServiceSid
        Set-I2eDirectoryAcl -Path $QualificationRoot -ServiceSid $serviceSid
        Set-I2eDirectoryAcl -Path $outputRoot -ServiceSid $serviceSid
        $config = Get-I2fServiceConfig -Scope $scope -OutputRoot $outputRoot -ServiceSid $serviceSid `
            -AmdCliPreflight $identityCheck.current_identity
        Write-I2fJson -Path $ConfigPath -Value $config
        Write-I2fJson -Path (Join-Path $outputRoot 'I2F-CONFIG.json') -Value $config

        $rightState = Assert-I2fRightAbsent -ServiceSid $serviceSid
        Write-I2fJson -Path (Join-Path $outputRoot 'I2F-LSA-BEFORE.json') -Value $rightState
        Add-I2eExactServiceProfileRight -ServiceSid $serviceSid
        $rightAdded = $true
        $afterAdd = Get-I2fRightState -ServiceSid $serviceSid
        if (-not $afterAdd.direct_present -or -not $afterAdd.assignment_present) {
            throw 'I2F exact-right dual verification failed after LSA assignment.'
        }
        Write-I2fJson -Path (Join-Path $outputRoot 'I2F-LSA-AFTER-ADD.json') -Value $afterAdd

        Invoke-I2eSc -Arguments @('start', $ServiceName) | Out-Null
        $result = Wait-I2fServiceEvidence -OutputRoot $outputRoot
    } catch {
        $primaryExperimentError = $_.Exception.Message
    }
}
finally {
    $cleanupResult = Invoke-I2fCleanup -OutputRoot $outputRoot -ServiceCreated $serviceCreated `
        -RightAdded $rightAdded -ServiceName $ServiceName -BrokerArtifactPath $ArtifactPath `
        -ServiceSid $serviceSid -AmdCliPath $amdCliPath `
        -PrimaryExperimentError $primaryExperimentError
}

if ($null -ne $primaryExperimentError) {
    $cleanupError = [string](Get-I2fPropertyValue -Object $cleanupResult.state -Name 'cleanup_error' -Default '')
    $evidenceError = [string](Get-I2fPropertyValue -Object $cleanupResult -Name 'evidence_write_error' -Default '')
    if (-not [string]::IsNullOrWhiteSpace($cleanupError) -or -not [string]::IsNullOrWhiteSpace($evidenceError)) {
        throw ('PRIMARY_EXPERIMENT_ERROR: {0}; CLEANUP_ERROR: {1}; ROLLBACK_EVIDENCE_WRITE_ERROR: {2}' -f
            $primaryExperimentError, $cleanupError, $evidenceError)
    }
    throw ('PRIMARY_EXPERIMENT_ERROR: {0}' -f $primaryExperimentError)
}

if ($cleanupResult.state.cleanup_required -and -not $cleanupResult.state.full_rollback_verified) {
    $cleanupError = [string](Get-I2fPropertyValue -Object $cleanupResult.state -Name 'cleanup_error' -Default '')
    $evidenceError = [string](Get-I2fPropertyValue -Object $cleanupResult -Name 'evidence_write_error' -Default '')
    throw ('I2F cleanup failed closed; CLEANUP_ERROR: {0}; ROLLBACK_EVIDENCE_WRITE_ERROR: {1}' -f
        $cleanupError, $evidenceError)
}

Remove-Item -LiteralPath $ConfigPath -Force -ErrorAction SilentlyContinue
Write-Host "I2F_SCOPE=$scope"
Write-Host "I2F_RESULT=$($result.availability)"
Write-Host 'I2F_REAL_EXECUTION_COMPLETED=true'
