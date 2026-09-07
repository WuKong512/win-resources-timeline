#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$I2fServiceName = 'ResourceTimelineAmdSystemProfileEnableQualification'
$I2fServiceAccount = 'NT AUTHORITY\LOCAL SERVICE'
$I2fServiceAccountSid = 'S-1-5-19'
$I2fServiceSidAccount = "NT SERVICE\$I2fServiceName"
$I2fRequiredRight = 'SeSystemProfilePrivilege'
$I2fForbiddenPrivileges = @('SeProfileSingleProcessPrivilege', 'SeDebugPrivilege')
$I2fFixedArguments = @('timechart', '--list')
$I2fOutputSubdirectory = 'ResourceTimeline\qualification\amd-system-profile-enable'
$I2fAuthoritativeScope = 'f68bf4d3d36547a0ba753cff489bb6eb'
$I2fRealGateConsumed = $true
$I2fRealRerunAllowed = $false
$I2fRealCleanupAllowed = $false
$I2fAuthoritativeRollbackComplete = $true

function Get-I2fExperimentPlan {
    param([Parameter(Mandatory = $true)][string]$ArtifactSha256)

    [ordered]@{
        schema = 'amd-i2f-service-profile-experiment-plan/v1'
        qualification_only = $true
        service_name = $I2fServiceName
        service_account = $I2fServiceAccount
        service_account_sid = $I2fServiceAccountSid
        service_sid_account = $I2fServiceSidAccount
        right = $I2fRequiredRight
        fixed_cli_arguments = $I2fFixedArguments
        sampling = $false
        intentional_variable = 'SeSystemProfilePrivilege DISABLED -> ENABLED via AdjustTokenPrivileges'
        forbidden_account_wide_mutation = 'S-1-5-19'
        forbidden_group_mutation = 'S-1-5-32-544'
        forbidden_privileges = $I2fForbiddenPrivileges
        artifact_sha256 = $ArtifactSha256
        authoritative_scope = $I2fAuthoritativeScope
        gate_consumed = $I2fRealGateConsumed
        real_rerun = if ($I2fRealRerunAllowed) { 'ALLOWED' } else { 'FORBIDDEN' }
        experiment_status = 'RETIRED_REAL_GATE_CONSUMED'
        rollback = [ordered]@{
            api = 'LsaRemoveAccountRights'
            all_rights = $false
            exact_sid = 'derived dedicated Service SID'
            exact_right = $I2fRequiredRight
            token_enablement = 'dies with the qualification service process'
        }
    }
}
function Get-I2fServiceConfig {
    param(
        [Parameter(Mandatory = $true)][string]$Scope,
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)]$AmdCliPreflight
    )

    [ordered]@{
        schema = 'amd-i2f-counter-config/v1'
        qualification_only = $true
        service_name = $I2fServiceName
        service_account = $I2fServiceAccount
        service_account_sid = $I2fServiceAccountSid
        service_sid = $ServiceSid
        service_sid_type = 'UNRESTRICTED'
        service_sid_type_verified = $true
        scope = $Scope
        output_root = $OutputRoot
        expected_amd_cli_path = [string]$AmdCliPreflight.path
        expected_amd_cli_sha256 = [string]$AmdCliPreflight.sha256
        expected_amd_cli_architecture = [string]$AmdCliPreflight.architecture
        fixed_cli_arguments = $I2fFixedArguments
        sampling = $false
        counter_discovery_only = $true
        self_enable_privilege = $I2fRequiredRight
    }
}

function Compare-I2fTokenStateFixture {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After
    )

    $beforeEnabled = @($Before.enabled_privileges | Sort-Object -Unique)
    $afterEnabled = @($After.enabled_privileges | Sort-Object -Unique)
    $beforeDisabled = @($Before.disabled_privileges | Sort-Object -Unique)
    $afterDisabled = @($After.disabled_privileges | Sort-Object -Unique)
    $addedEnabled = @($afterEnabled | Where-Object { $beforeEnabled -notcontains $_ })
    $removedEnabled = @($beforeEnabled | Where-Object { $afterEnabled -notcontains $_ })
    $addedDisabled = @($afterDisabled | Where-Object { $beforeDisabled -notcontains $_ })
    $removedDisabled = @($beforeDisabled | Where-Object { $afterDisabled -notcontains $_ })
    $identityFields = @('account_sid', 'service_sid', 'session_id', 'process_architecture')
    $identityMismatches = @($identityFields | Where-Object {
            ([string]$Before.$_) -cne ([string]$After.$_)
        })
    $administratorsPresent = @($After.token_groups_relevant_to_access | Where-Object {
            ([string]$_ -split ':', 2)[0] -ieq 'S-1-5-32-544'
        }).Count -gt 0
    $forbiddenEnabled = @($I2fForbiddenPrivileges | Where-Object {
            $privilege = $_
            @($afterEnabled | Where-Object { $_ -ieq $privilege }).Count -gt 0
        })
    [pscustomobject]@{
        changed_privilege = $I2fRequiredRight
        before = 'DISABLED'
        after = 'ENABLED'
        added_enabled_privileges = $addedEnabled
        removed_enabled_privileges = $removedEnabled
        added_disabled_privileges = $addedDisabled
        removed_disabled_privileges = $removedDisabled
        identity_mismatches = $identityMismatches
        administrators_sid_present = $administratorsPresent
        forbidden_enabled_privileges = $forbiddenEnabled
        exact_one_intentional_privilege_state_change = ($addedEnabled.Count -eq 1 -and
            $addedEnabled[0] -ieq $I2fRequiredRight -and
            $removedEnabled.Count -eq 0 -and
            $addedDisabled.Count -eq 0 -and
            $removedDisabled.Count -eq 1 -and
            $removedDisabled[0] -ieq $I2fRequiredRight)
        pass = ($identityMismatches.Count -eq 0 -and
            -not $administratorsPresent -and
            $forbiddenEnabled.Count -eq 0 -and
            $addedEnabled.Count -eq 1 -and
            $addedEnabled[0] -ieq $I2fRequiredRight -and
            $removedEnabled.Count -eq 0 -and
            $addedDisabled.Count -eq 0 -and
            $removedDisabled.Count -eq 1 -and
            $removedDisabled[0] -ieq $I2fRequiredRight)
    }
}

function Test-I2fPreEnableTokenGateFixture {
    param([Parameter(Mandatory = $true)]$Context, [Parameter(Mandatory = $true)][string]$ServiceSid)

    $enabled = @($Context.enabled_privileges)
    $disabled = @($Context.disabled_privileges)
    $groups = @($Context.token_groups_relevant_to_access)
    $profileEnabled = @($enabled | Where-Object { $_ -ieq $I2fRequiredRight }).Count -gt 0
    $profileDisabled = @($disabled | Where-Object { $_ -ieq $I2fRequiredRight }).Count -gt 0
    $admins = @($groups | Where-Object { ([string]$_ -split ':', 2)[0] -ieq 'S-1-5-32-544' }).Count -gt 0
    $forbidden = @($I2fForbiddenPrivileges | Where-Object {
            $privilege = $_
            @($enabled | Where-Object { $_ -ieq $privilege }).Count -gt 0
        })
    [pscustomobject]@{
        account_sid = [string]$Context.account_sid
        service_sid = $ServiceSid
        session_id = $Context.session_id
        process_architecture = [string]$Context.process_architecture
        administrators_sid_present = $admins
        se_system_profile_privilege_present = ($profileEnabled -or $profileDisabled)
        se_system_profile_privilege_enabled = $profileEnabled
        se_system_profile_privilege_disabled = $profileDisabled
        forbidden_enabled_privileges = $forbidden
        pass = ([bool]$Context.context_valid -and
            [string]$Context.account_sid -ieq $I2fServiceAccountSid -and
            [int]$Context.session_id -eq 0 -and
            [string]$Context.process_architecture -ceq 'x64' -and
            [bool]$Context.service_sid_present -and
            -not $admins -and
            $profileDisabled -and
            -not $profileEnabled -and
            $forbidden.Count -eq 0)
    }
}

function Assert-I2fFixedArguments {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    if ($Arguments.Count -ne 2 -or $Arguments[0] -cne 'timechart' -or $Arguments[1] -cne '--list') {
        throw 'I2F accepts only the fixed non-sampling command: timechart --list.'
    }
}

function Get-I2fPropertyValue {
    param(
        [AllowNull()][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][object]$Default = $null
    )

    if ($null -eq $Object) { return $Default }
    $property = @($Object.PSObject.Properties | Where-Object Name -eq $Name | Select-Object -First 1)
    if ($property.Count -eq 1) { return $property[0].Value }
    return $Default
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

function New-I2fCleanupState {
    [pscustomobject]@{
        cleanup_phase = 'NOT_STARTED'
        cleanup_required = $false
        service_stop_attempted = $false
        service_stop_verified = $false
        service_state_after_stop = 'UNKNOWN'
        service_pid_after_stop = $null
        owned_process_check_attempted = $false
        owned_process_check_verified = $false
        owned_broker_process_count_after_stop = $null
        amd_cli_process_count_after_stop = $null
        effective_token_teardown_verified = $false
        policy_pre_remove_readback_verified = $false
        policy_remove_attempted = $false
        lsa_remove_account_rights_calls = 0
        policy_remove_skipped_reason = $null
        policy_right_removed = $false
        policy_rollback_verified = $false
        policy_rollback_state_source = $null
        service_registration_remove_attempted = $false
        service_registration_removed = $false
        full_rollback_verified = $false
        rollback_verified = $false
        direct_verification = $null
        assignment_verification = $null
        policy_readback_error = $null
        cleanup_error = $null
        primary_experiment_error = $null
        rollback_evidence_write_error = $null
    }
}

function Get-I2fCleanupDecision {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][bool]$ServicePresent,
        [Parameter(Mandatory = $true)][string]$ServiceState,
        [AllowNull()][Nullable[Int64]]$ServiceProcessId,
        [Parameter(Mandatory = $true)][bool]$StopAttempted,
        [Parameter(Mandatory = $true)][bool]$StopVerified,
        [Parameter(Mandatory = $true)][bool]$ProcessCheckAttempted,
        [Parameter(Mandatory = $true)][bool]$ProcessCheckVerified,
        [AllowNull()][Nullable[Int64]]$BrokerCount,
        [AllowNull()][Nullable[Int64]]$AmdCliCount,
        [Parameter(Mandatory = $true)][bool]$RightDirectPresent,
        [Parameter(Mandatory = $true)][bool]$RightAssignmentPresent,
        [Parameter(Mandatory = $true)][bool]$RightReadbackVerified,
        [Parameter(Mandatory = $true)][bool]$PolicyRollbackVerified,
        [Parameter(Mandatory = $true)][bool]$ServiceRegistrationPresent,
        [Parameter(Mandatory = $true)][bool]$PolicyRollbackPreviouslyVerified
    )

    $countsObserved = $ProcessCheckAttempted -and $ProcessCheckVerified -and
        $null -ne $BrokerCount -and $null -ne $AmdCliCount
    $brokerCountValue = if ($null -eq $BrokerCount) { $null } else { [int64]$BrokerCount }
    $amdCliCountValue = if ($null -eq $AmdCliCount) { $null } else { [int64]$AmdCliCount }
    $effectiveTokenTeardownVerified = $StopVerified -and $countsObserved -and
        $brokerCountValue -eq 0 -and $amdCliCountValue -eq 0
    $rightPresent = $RightDirectPresent -or $RightAssignmentPresent
    $policyStateDrift = $PolicyRollbackPreviouslyVerified -and $rightPresent
    $policyRemoveAllowed = $effectiveTokenTeardownVerified -and $RightReadbackVerified -and
        -not $PolicyRollbackPreviouslyVerified
    $policyRemoveRequired = $policyRemoveAllowed -and $rightPresent
    $serviceDeleteAllowed = $effectiveTokenTeardownVerified -and
        $RightReadbackVerified -and $PolicyRollbackVerified
    $fullRollbackPossible = $effectiveTokenTeardownVerified -and
        $PolicyRollbackVerified -and -not $ServiceRegistrationPresent

    $failureClassification = 'NONE'
    if (-not $StopVerified) {
        $failureClassification = 'SERVICE_STOP_NOT_VERIFIED'
    } elseif (-not $ProcessCheckVerified) {
        $failureClassification = 'PROCESS_CHECK_NOT_VERIFIED'
    } elseif (-not $countsObserved) {
        $failureClassification = 'PROCESS_COUNTS_UNKNOWN'
    } elseif ($countsObserved -and ($brokerCountValue -gt 0 -or $amdCliCountValue -gt 0)) {
        $failureClassification = 'OWNED_PROCESS_PRESENT'
    } elseif (-not $RightReadbackVerified) {
        $failureClassification = 'POLICY_READBACK_UNAVAILABLE'
    } elseif ($policyStateDrift) {
        $failureClassification = 'POLICY_ROLLBACK_STATE_DRIFT'
    } elseif (-not $PolicyRollbackVerified) {
        $failureClassification = 'POLICY_ROLLBACK_INCOMPLETE'
    } elseif ($ServiceRegistrationPresent) {
        $failureClassification = 'SERVICE_REGISTRATION_REMAINS'
    }

    [pscustomobject]@{
        effective_token_teardown_verified = $effectiveTokenTeardownVerified
        policy_remove_allowed = $policyRemoveAllowed
        policy_remove_required = $policyRemoveRequired
        policy_state_drift = $policyStateDrift
        service_delete_allowed = $serviceDeleteAllowed
        full_rollback_possible = $fullRollbackPossible
        failure_classification = $failureClassification
        service_present = $ServicePresent
        service_state = $ServiceState
        service_process_id = $ServiceProcessId
        stop_attempted = $StopAttempted
        stop_verified = $StopVerified
        process_check_attempted = $ProcessCheckAttempted
        process_check_verified = $ProcessCheckVerified
        broker_count = $BrokerCount
        amd_cli_count = $AmdCliCount
        right_readback_verified = $RightReadbackVerified
        policy_rollback_verified = $PolicyRollbackVerified
        service_registration_present = $ServiceRegistrationPresent
    }
}

function Get-I2fOwnedProcessEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$BrokerArtifactPath,
        [Parameter(Mandatory = $true)][string]$AmdCliPath
    )

    $brokerPath = [IO.Path]::GetFullPath($BrokerArtifactPath)
    $amdPath = [IO.Path]::GetFullPath($AmdCliPath)
    try {
        $brokerProcesses = @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue |
            Where-Object {
                try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $brokerPath) } catch { $false }
            })
        $amdProcesses = @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue |
            Where-Object {
                try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $amdPath) } catch { $false }
            })
        [pscustomobject]@{
            owned_broker_process_count = [int64]$brokerProcesses.Count
            amd_cli_process_count = [int64]$amdProcesses.Count
        }
    } catch {
        throw ('I2F owned-process enumeration failed: {0}' -f $_.Exception.Message)
    }
}

function Write-I2fRollbackEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][bool]$RightAddedByExperiment,
        [Parameter(Mandatory = $true)]$State,
        [AllowNull()][object]$RightState
    )

    $directVerification = Get-I2fPropertyValue -Object $RightState -Name 'direct'
    $assignmentVerification = Get-I2fPropertyValue -Object $RightState -Name 'assigned'
    $evidence = [ordered]@{
        schema = 'amd-i2f-rollback/v2'
        qualification_only = $true
        right = $I2fRequiredRight
        right_added_by_experiment = $RightAddedByExperiment
        all_rights = $false
        service_stop_attempted = [bool](Get-I2fPropertyValue -Object $State -Name 'service_stop_attempted' -Default $false)
        service_stop_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'service_stop_verified' -Default $false)
        service_state_after_stop = Get-I2fPropertyValue -Object $State -Name 'service_state_after_stop' -Default 'UNKNOWN'
        service_pid_after_stop = Get-I2fPropertyValue -Object $State -Name 'service_pid_after_stop'
        owned_process_check_attempted = [bool](Get-I2fPropertyValue -Object $State -Name 'owned_process_check_attempted' -Default $false)
        owned_process_check_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'owned_process_check_verified' -Default $false)
        owned_broker_process_count_after_stop = Get-I2fPropertyValue -Object $State -Name 'owned_broker_process_count_after_stop'
        amd_cli_process_count_after_stop = Get-I2fPropertyValue -Object $State -Name 'amd_cli_process_count_after_stop'
        effective_token_teardown_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'effective_token_teardown_verified' -Default $false)
        policy_pre_remove_readback_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'policy_pre_remove_readback_verified' -Default $false)
        policy_remove_attempted = [bool](Get-I2fPropertyValue -Object $State -Name 'policy_remove_attempted' -Default $false)
        lsa_remove_account_rights_calls = [int64](Get-I2fPropertyValue -Object $State -Name 'lsa_remove_account_rights_calls' -Default 0)
        policy_remove_skipped_reason = Get-I2fPropertyValue -Object $State -Name 'policy_remove_skipped_reason'
        policy_right_removed = [bool](Get-I2fPropertyValue -Object $State -Name 'policy_right_removed' -Default $false)
        policy_rollback_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'policy_rollback_verified' -Default $false)
        policy_rollback_state_source = Get-I2fPropertyValue -Object $State -Name 'policy_rollback_state_source'
        direct_verification = $directVerification
        assignment_verification = $assignmentVerification
        policy_readback_error = Get-I2fPropertyValue -Object $State -Name 'policy_readback_error'
        service_registration_remove_attempted = [bool](Get-I2fPropertyValue -Object $State -Name 'service_registration_remove_attempted' -Default $false)
        service_registration_removed = [bool](Get-I2fPropertyValue -Object $State -Name 'service_registration_removed' -Default $false)
        full_rollback_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'full_rollback_verified' -Default $false)
        rollback_verified = [bool](Get-I2fPropertyValue -Object $State -Name 'rollback_verified' -Default $false)
        cleanup_required = [bool](Get-I2fPropertyValue -Object $State -Name 'cleanup_required' -Default $false)
        cleanup_phase = Get-I2fPropertyValue -Object $State -Name 'cleanup_phase' -Default 'UNKNOWN'
        cleanup_error = Get-I2fPropertyValue -Object $State -Name 'cleanup_error'
        primary_experiment_error = Get-I2fPropertyValue -Object $State -Name 'primary_experiment_error'
        rollback_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    try {
        New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
        [IO.File]::WriteAllText(
            (Join-Path $OutputRoot 'I2F-ROLLBACK.json'),
            ($evidence | ConvertTo-Json -Depth 30),
            [Text.UTF8Encoding]::new($false))
        [pscustomobject]@{ success = $true; path = (Join-Path $OutputRoot 'I2F-ROLLBACK.json'); error = $null }
    } catch {
        [pscustomobject]@{ success = $false; path = (Join-Path $OutputRoot 'I2F-ROLLBACK.json'); error = $_.Exception.Message }
    }
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

function Get-I2fSafeServiceSnapshot {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    try {
        Get-I2eServiceSnapshot -ServiceName $ServiceName
    } catch {
        [pscustomobject]@{
            present = $null
            state = 'UNKNOWN'
            process_id = $null
            start_name = $null
            error = $_.Exception.Message
        }
    }
}

function Invoke-I2fCleanup {
    [CmdletBinding()]
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
        try {
            $null = Stop-I2eService -ServiceName $ServiceName
        } catch {
            Add-I2fCleanupError -State $state -Message $_.Exception.Message
        }
        $serviceSnapshot = Get-I2fSafeServiceSnapshot -ServiceName $ServiceName
    }

    if ($null -eq $serviceSnapshot.present) {
        $state.service_stop_verified = $false
        $state.service_state_after_stop = 'UNKNOWN'
        $state.service_pid_after_stop = $null
    } elseif (-not $serviceSnapshot.present) {
        $state.service_state_after_stop = 'ABSENT'
        $state.service_pid_after_stop = 0L
        $state.service_stop_verified = $true
    } else {
        $state.service_state_after_stop = [string]$serviceSnapshot.state
        $state.service_pid_after_stop = [int64]$serviceSnapshot.process_id
        $state.service_stop_verified = $serviceSnapshot.state -eq 'Stopped' -and
            $serviceSnapshot.process_id -eq 0
    }

    if (-not $state.service_stop_verified) {
        if ([string]::IsNullOrWhiteSpace([string]$state.cleanup_error)) {
            Add-I2fCleanupError -State $state -Message 'Service stop was not verified as Stopped/PID0 or service absence.'
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
            if ($policyDecision.policy_remove_required) {
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
            } else {
                $state.policy_remove_skipped_reason = 'ALREADY_ABSENT'
                $state.policy_rollback_state_source = 'FRESH_DUAL_READBACK'
                $state.policy_rollback_verified = $true
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
        $state.service_registration_removed = $null -ne $serviceSnapshot.present -and
            -not $serviceSnapshot.present
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
