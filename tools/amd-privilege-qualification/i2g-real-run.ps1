#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$AuthorizationToken
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# This file is an explicitly authorized, one-shot qualification runner.  It is not a reusable
# production privileged execution surface: the caller must supply the task token, the exact
# artifact/AMD identities are pinned below, and every mutation is journaled before it occurs.
. (Join-Path $PSScriptRoot 'i2g-runtime-contract.ps1')

if ($I2gRealGateConsumed) {
    throw 'I2G_REAL_EXECUTION_NOT_AUTHORIZED: I2G_REAL_GATE_CONSUMED=true; this task authorization is consumed.'
}

. (Join-Path $PSScriptRoot 'i2e-runtime-library.ps1')

$I2gAuthorizationToken = 'AMD-PRIVILEGE-I2G-REAL-QUALIFICATION'
$I2gServiceName = 'ResourceTimelineAmdProfileSingleProcessQualification'
$I2gServiceAccount = 'NT AUTHORITY\LocalService'
$I2gServiceAccountSid = 'S-1-5-19'
$I2gServiceSidAccount = "NT SERVICE\$I2gServiceName"
$I2gOutputSubdirectory = 'ResourceTimeline\qualification\amd-i2g-real'
$I2gControlRight = 'SeSystemProfilePrivilege'
$I2gTreatmentRight = 'SeProfileSingleProcessPrivilege'
$I2gFixedAmdCliPath = 'D:\apps\AMDuProf\bin\AMDuProfCLI.exe'
$I2gFixedAmdCliVersion = '5.3.521.0'
$I2gFixedAmdCliArchitecture = 'x64'
$I2gFixedAmdCliSha256 = 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC'
$I2gMaxControlRuns = 1
$I2gMaxTreatmentRuns = 1
$I2gMaxTotalRuns = 2

if ($AuthorizationToken -cne $I2gAuthorizationToken) {
    throw 'I2G_REAL_EXECUTION_NOT_AUTHORIZED: exact task authorization token is required.'
}
if ($env:I2G_REAL_RUN_AUTHORIZATION -cne 'GRANTED_FOR_THIS_TASK_ONLY') {
    throw 'I2G_REAL_EXECUTION_NOT_AUTHORIZED: task authorization environment marker is required.'
}

$ArtifactPath = Join-Path $PSScriptRoot $I2gHarnessArtifactRelativePath
$QualificationRoot = Join-Path $env:ProgramData $I2gOutputSubdirectory
$ConfigPath = Join-Path $QualificationRoot 'I2G-CONFIG.json'
$ServiceCreated = $false
$ServiceSid = $null
$SystemProfileAddedByRun = $false
$ProfileSingleAddedByRun = $false
$AclSddlBefore = $null
$AclWasMutated = $false
$RunRoot = $null
$RunId = $null
$ControlRuns = 0
$TreatmentRuns = 0
$ControlResult = 'NOT_RUN'
$TreatmentResult = 'NOT_RUN'
$PairedResult = 'INVALID_NO_CAUSAL_INTERPRETATION'
$CausalInterpretationValid = $false
$TreatmentAllowed = $false
$TreatmentAllowedByScientificGate = $false
$TreatmentPolicyMutationStarted = $false
$TreatmentPolicyMutationCompleted = $false
$TreatmentServicePhaseStarted = $false
$TreatmentDiscoverySpawnIntentDurable = $false
$TreatmentDiscoveryStarted = $false
$TreatmentDiscoveryCompleted = $false
$HarnessRuntimeFailure = $false
$FailureClass = 'NONE'
$ScientificResult = 'NOT_OBTAINED'
$PrimaryError = $null
$CleanupError = $null
$Rollback = $null
$ControlPolicyBefore = $null
$TreatmentPolicyBefore = $null
$SystemProfilePreexisting = $false
$ProfileSinglePreexisting = $false
$AmdCliIdentity = $null
$HarnessIdentity = $null
$ControlTokenGate = $false
$TreatmentTokenGate = $false
$ControlTeardownPass = $false
$FinalMachineState = $null
$PairedConfigPass = $false
$PairedTokenPass = $false
$TokenComparison = $null

$script:I2gState = $null
$script:I2gStateSequence = 0

function Write-I2gAtomicJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    $temp = Join-Path $parent ('.pending-{0}.json' -f ([Guid]::NewGuid().ToString('N')))
    try {
        [IO.File]::WriteAllText($temp, ($Value | ConvertTo-Json -Depth 50), [Text.UTF8Encoding]::new($false))
        [IO.File]::Move($temp, $Path)
    }
    catch {
        if (Test-Path -LiteralPath $temp -PathType Leaf) {
            Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
        }
        throw
    }
}

function Write-I2gPlaceholderIfMissing {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Write-I2gAtomicJson -Path $Path -Value $Value
    }
}

function Read-I2gJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Save-I2gState {
    if ($null -eq $script:I2gState) { return }
    $script:I2gState.updated_at_utc = [DateTime]::UtcNow.ToString('o')
    $path = Join-Path $RunRoot ('STATE-{0:D4}.json' -f $script:I2gStateSequence)
    Write-I2gAtomicJson -Path $path -Value $script:I2gState
    $script:I2gStateSequence++
}

function Set-I2gState {
    param([Parameter(Mandatory = $true)][string]$State)
    $script:I2gState.state = $State
    Save-I2gState
}

function New-I2gTreatmentConfig {
    param([Parameter(Mandatory = $true)]$ControlConfig)

    # OrderedDictionary has no usable clone method in Windows PowerShell 5.1.  Keep this
    # copy explicit so the paired configuration remains auditable and deterministic.
    [ordered]@{
        schema = $ControlConfig.schema
        qualification_only = $ControlConfig.qualification_only
        service_name = $ControlConfig.service_name
        service_account = $ControlConfig.service_account
        service_account_sid = $ControlConfig.service_account_sid
        service_sid = $ControlConfig.service_sid
        scope = $ControlConfig.scope
        output_root = $ControlConfig.output_root
        phase = 'TREATMENT'
        expected_profile_single_process_privilege = $true
        expected_amd_cli_path = $ControlConfig.expected_amd_cli_path
        expected_amd_cli_sha256 = $ControlConfig.expected_amd_cli_sha256
        expected_amd_cli_version = $ControlConfig.expected_amd_cli_version
        expected_amd_cli_architecture = $ControlConfig.expected_amd_cli_architecture
        harness_artifact_sha256 = $ControlConfig.harness_artifact_sha256
    }
}

function Get-I2gTreatmentPlaceholderReason {
    param(
        [Parameter(Mandatory = $true)][bool]$TreatmentAllowedByScientificGate,
        [Parameter(Mandatory = $true)][bool]$TreatmentDiscoveryStarted,
        [Parameter(Mandatory = $true)][bool]$TreatmentDiscoveryCompleted,
        [Parameter(Mandatory = $true)][bool]$HarnessRuntimeFailure
    )

    if (-not $TreatmentAllowedByScientificGate) {
        return 'NOT_ALLOWED_BY_SCIENTIFIC_GATE'
    }
    if (-not $TreatmentDiscoveryStarted) {
        return 'ALLOWED_BUT_NOT_STARTED_DUE_TO_HARNESS_FAILURE'
    }
    if (-not $TreatmentDiscoveryCompleted) {
        return 'STARTED_BUT_EVIDENCE_NOT_COMPLETED'
    }
    if ($HarnessRuntimeFailure) {
        return 'STARTED_BUT_EVIDENCE_NOT_COMPLETED'
    }
    return 'STARTED_BUT_EVIDENCE_NOT_COMPLETED'
}

function Resolve-I2gFailureClass {
    param(
        [Parameter(Mandatory = $true)][string]$ErrorMessage,
        [Parameter(Mandatory = $true)][bool]$TreatmentAllowedByScientificGate,
        [Parameter(Mandatory = $true)][int]$ControlRuns,
        [Parameter(Mandatory = $true)][string]$ControlResult
    )

    if ($ErrorMessage -match '^(INVALID_NO_CAUSAL_INTERPRETATION|CONTROL_DRIFT|INVALID_CONFIGURATION_DELTA|INVALID_TOKEN_DELTA|TOKEN_GATE_FAILED|IDENTITY_MISMATCH|PROCESS_OWNERSHIP_FAILED|TIMEOUT|DISCOVERY_FAILED|CLEANUP_FAILED)(:|$)') {
        return 'SCIENTIFIC_INVALIDATION'
    }
    if (-not $TreatmentAllowedByScientificGate -and $ControlRuns -gt 0 -and $ControlResult -ne 'POWER_UNAVAILABLE') {
        return 'SCIENTIFIC_INVALIDATION'
    }
    return 'HARNESS_RUNTIME_ERROR'
}

function Get-I2gRightSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string]$Sid,
        [Parameter(Mandatory = $true)][string]$Right
    )
    try {
        Initialize-I2eLsaMutationType
        $direct = [I2eLsaMutation]::EnumerateAccountRightsDetailed($Sid)
        $assigned = @([I2eLsaMutation]::EnumerateAccountsWithUserRight($Right))
        [ordered]@{
            label = $Label
            account_sid = $Sid
            right = $Right
            direct_rights = @($direct.Rights)
            account_object_state = [string]$direct.AccountObjectState
            assigned_principals = $assigned
            direct_present = @($direct.Rights | Where-Object { $_ -ieq $Right }).Count -gt 0
            assignment_present = @($assigned | Where-Object { $_ -ieq $Sid }).Count -gt 0
            status = 'READ'
            source = 'LsaEnumerateAccountRights + LsaEnumerateAccountsWithUserRight'
        }
    }
    catch {
        [ordered]@{
            label = $Label
            account_sid = $Sid
            right = $Right
            direct_rights = @()
            account_object_state = 'UNKNOWN'
            assigned_principals = @()
            direct_present = $false
            assignment_present = $false
            status = 'UNAVAILABLE'
            error = $_.Exception.Message
        }
    }
}

function Add-I2gExactRight {
    param([Parameter(Mandatory = $true)][string]$Sid, [Parameter(Mandatory = $true)][string]$Right)
    if ($Sid -notmatch '^S-1-5-80-') { throw "Refusing to mutate a non-Service SID: $Sid" }
    Initialize-I2eLsaMutationType
    [I2eLsaMutation]::AddExactRight($Sid, $Right)
}

function Remove-I2gExactRight {
    param([Parameter(Mandatory = $true)][string]$Sid, [Parameter(Mandatory = $true)][string]$Right)
    if ($Sid -notmatch '^S-1-5-80-') { throw "Refusing to mutate a non-Service SID: $Sid" }
    Initialize-I2eLsaMutationType
    [I2eLsaMutation]::RemoveExactRight($Sid, $Right)
}

function Test-I2gExactPolicy {
    param(
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)][string[]]$ExpectedRights
    )
    if ($Snapshot.status -ne 'READ') { return $false }
    $actual = @($Snapshot.direct_rights | Sort-Object -Unique)
    $expected = @($ExpectedRights | Sort-Object -Unique)
    if ($actual.Count -ne $expected.Count) { return $false }
    for ($index = 0; $index -lt $expected.Count; $index++) {
        if ($actual[$index] -cne $expected[$index]) { return $false }
    }
    if ($expected -contains ([string]$Snapshot.right) -and -not [bool]$Snapshot.assignment_present) {
        return $false
    }
    $true
}

function Get-I2gAmdCliIdentity {
    $exists = Test-Path -LiteralPath $I2gFixedAmdCliPath -PathType Leaf
    if (-not $exists) {
        return [ordered]@{
            path = $I2gFixedAmdCliPath
            exists = $false
            sha256 = $null
            version = $null
            architecture = $null
            signature_status = $null
            identity_pass = $false
        }
    }
    $item = Get-Item -LiteralPath $I2gFixedAmdCliPath
    $signature = Get-AuthenticodeSignature -LiteralPath $I2gFixedAmdCliPath
    $hash = (Get-FileHash -LiteralPath $I2gFixedAmdCliPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $architecture = Get-I2gHarnessArtifactArchitecture -Path $I2gFixedAmdCliPath
    $version = [string]$item.VersionInfo.FileVersion
    [ordered]@{
        path = $I2gFixedAmdCliPath
        exists = $true
        sha256 = $hash
        version = $version
        architecture = $architecture
        signature_status = [string]$signature.Status
        identity_pass = ($hash -ceq $I2gFixedAmdCliSha256 -and
            $version -ceq $I2gFixedAmdCliVersion -and
            $architecture -ceq $I2gFixedAmdCliArchitecture)
    }
}

function Get-I2gPrivilegeState {
    param([Parameter(Mandatory = $true)]$Context, [Parameter(Mandatory = $true)][string]$Privilege)
    if (@($Context.enabled_privileges | Where-Object { $_ -ieq $Privilege }).Count -gt 0) {
        return 'PRESENT + ENABLED'
    }
    if (@($Context.disabled_privileges | Where-Object { $_ -ieq $Privilege }).Count -gt 0) {
        return 'PRESENT + DISABLED'
    }
    'ABSENT'
}

function Compare-I2gTokenPair {
    param([Parameter(Mandatory = $true)]$Control, [Parameter(Mandatory = $true)]$Treatment)
    $controlContext = $Control.context
    $treatmentContext = $Treatment.context
    $identityFields = @('account_sid', 'service_sid', 'service_sid_type', 'session_id', 'process_architecture')
    $identityMismatches = @($identityFields | Where-Object {
        ([string]$controlContext.$_) -cne ([string]$treatmentContext.$_)
    })
    $controlEnabled = @($controlContext.enabled_privileges | Sort-Object -Unique)
    $treatmentEnabled = @($treatmentContext.enabled_privileges | Sort-Object -Unique)
    $controlDisabled = @($controlContext.disabled_privileges | Sort-Object -Unique)
    $treatmentDisabled = @($treatmentContext.disabled_privileges | Sort-Object -Unique)
    $enabledAdded = @($treatmentEnabled | Where-Object { $controlEnabled -notcontains $_ })
    $enabledRemoved = @($controlEnabled | Where-Object { $treatmentEnabled -notcontains $_ })
    $disabledAdded = @($treatmentDisabled | Where-Object { $controlDisabled -notcontains $_ })
    $disabledRemoved = @($controlDisabled | Where-Object { $treatmentDisabled -notcontains $_ })
    $withoutTarget = {
        param([string[]]$Values)
        @($Values | Where-Object { $_ -ine $I2gTreatmentRight } | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object -Unique)
    }
    $invalid = @()
    if ($identityMismatches.Count -gt 0) { $invalid += 'identity' }
    if (-not [bool]$controlContext.context_valid -or -not [bool]$treatmentContext.context_valid) { $invalid += 'context_valid' }
    $controlGroupsComparable = (@($controlContext.token_groups_relevant_to_access) -join '|')
    $treatmentGroupsComparable = (@($treatmentContext.token_groups_relevant_to_access) -join '|')
    if ($controlGroupsComparable -cne $treatmentGroupsComparable) { $invalid += 'token_groups' }
    $controlEnabledComparable = ((& $withoutTarget $controlEnabled) -join '|')
    $treatmentEnabledComparable = ((& $withoutTarget $treatmentEnabled) -join '|')
    if ($controlEnabledComparable -cne $treatmentEnabledComparable) { $invalid += 'enabled_privileges_except_treatment' }
    $controlDisabledComparable = ((& $withoutTarget $controlDisabled) -join '|')
    $treatmentDisabledComparable = ((& $withoutTarget $treatmentDisabled) -join '|')
    if ($controlDisabledComparable -cne $treatmentDisabledComparable) { $invalid += 'disabled_privileges_except_treatment' }
    if ($enabledAdded.Count -ne 1 -or $enabledAdded[0] -ine $I2gTreatmentRight) { $invalid += 'enabled_delta' }
    if ($enabledRemoved.Count -ne 0) { $invalid += 'enabled_removed' }
    if ($disabledAdded.Count -ne 0) { $invalid += 'disabled_added' }
    if (@($disabledRemoved | Where-Object { $_ -ine $I2gTreatmentRight }).Count -ne 0) { $invalid += 'disabled_removed' }
    if ((Get-I2gPrivilegeState -Context $controlContext -Privilege $I2gControlRight) -ne 'PRESENT + ENABLED') { $invalid += 'control_system_profile' }
    if ((Get-I2gPrivilegeState -Context $treatmentContext -Privilege $I2gControlRight) -ne 'PRESENT + ENABLED') { $invalid += 'treatment_system_profile' }
    if ((Get-I2gPrivilegeState -Context $controlContext -Privilege $I2gTreatmentRight) -ne 'ABSENT') { $invalid += 'control_profile_single' }
    if ((Get-I2gPrivilegeState -Context $treatmentContext -Privilege $I2gTreatmentRight) -ne 'PRESENT + ENABLED') { $invalid += 'treatment_profile_single' }
    [ordered]@{
        schema = 'amd-i2g-paired-token-comparison/v1'
        qualification_only = $true
        allowed_delta = 'SeProfileSingleProcessPrivilege: ABSENT -> PRESENT + ENABLED'
        enabled_added = $enabledAdded
        enabled_removed = $enabledRemoved
        disabled_added = $disabledAdded
        disabled_removed = $disabledRemoved
        identity_mismatches = $identityMismatches
        invalid_fields = @($invalid | Sort-Object -Unique)
        pass = ($invalid.Count -eq 0)
    }
}

function Get-I2gOwnedProcessCounts {
    $broker = @(Get-I2eOwnedBrokerProcesses -ArtifactPath $ArtifactPath)
    $amd = @(Get-I2eOwnedAmdProcesses -ExpectedAmdCliPath $I2gFixedAmdCliPath)
    [ordered]@{
        owned_broker_process_count = [int64]$broker.Count
        amd_cli_process_count = [int64]$amd.Count
        owned_process_count = [int64]($broker.Count + $amd.Count)
    }
}

function Stop-I2gOwnedProcesses {
    $processes = @(
        @(Get-I2eOwnedBrokerProcesses -ArtifactPath $ArtifactPath)
        @(Get-I2eOwnedAmdProcesses -ExpectedAmdCliPath $I2gFixedAmdCliPath)
    )
    foreach ($process in $processes) {
        Stop-Process -Id $process.Id -Force -ErrorAction Stop
    }
    Start-Sleep -Milliseconds 250
}

function Invoke-I2gServicePhase {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('CONTROL', 'TREATMENT')][string]$Phase,
        [Parameter(Mandatory = $true)]$Config
    )
    if ($Phase -eq 'CONTROL' -and [int]$script:ControlRuns -ge $I2gMaxControlRuns) {
        throw 'CONTROL discovery budget is already consumed; retry is forbidden.'
    }
    if ($Phase -eq 'TREATMENT' -and [int]$script:TreatmentRuns -ge $I2gMaxTreatmentRuns) {
        throw 'TREATMENT discovery budget is already consumed; retry is forbidden.'
    }
    if ([int]$script:ControlRuns + [int]$script:TreatmentRuns -ge $I2gMaxTotalRuns) {
        throw 'Total discovery budget is already consumed; retry is forbidden.'
    }
    Write-I2gAtomicJson -Path $ConfigPath -Value $Config
    $script:I2gState.phase = $Phase
    $script:I2gState.service_start_intent_durable = $true
    $phaseState = if ($Phase -eq 'CONTROL') { 'ControlServiceRunning' } else { 'TreatmentServiceRunning' }
    if ($Phase -eq 'TREATMENT') {
        $script:I2gState.treatment_service_phase_started = $true
        $script:I2gState.treatment_service_phase_completed = $false
    }
    Set-I2gState -State $phaseState
    Invoke-I2eSc -Arguments @('start', $I2gServiceName) | Out-Null
    $serviceAfterStart = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
    $script:I2gState.service_start_observed = $serviceAfterStart.present
    $script:I2gState.service_pid = $serviceAfterStart.process_id
    Save-I2gState
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    $discoveryPath = Join-Path $RunRoot "$Phase-DISCOVERY.json"
    $errorPath = Join-Path $RunRoot 'I2G-SERVICE-HARNESS-ERROR.json'
    do {
        if ((Test-Path -LiteralPath $discoveryPath -PathType Leaf) -or (Test-Path -LiteralPath $errorPath -PathType Leaf)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $spawnIntentPath = Join-Path $RunRoot "$Phase-DISCOVERY-SPAWN-INTENT.json"
    $launchPath = Join-Path $RunRoot "$Phase-AMD-DISCOVERY-LAUNCH.json"
    $spawnIntent = Test-Path -LiteralPath $spawnIntentPath -PathType Leaf
    $launch = Test-Path -LiteralPath $launchPath -PathType Leaf
    if ($spawnIntent -or $launch) {
        if ($Phase -eq 'CONTROL') {
            $script:ControlRuns = 1
            $script:I2gState.control_runs = 1
        } else {
            $script:TreatmentRuns = 1
            $script:I2gState.treatment_runs = 1
            $script:I2gState.treatment_discovery_spawn_intent_durable = $true
            $script:I2gState.treatment_discovery_started = $true
            $script:TreatmentDiscoverySpawnIntentDurable = $true
            $script:TreatmentDiscoveryStarted = $true
        }
        $script:I2gState.total_runs = [int]$script:I2gState.control_runs + [int]$script:I2gState.treatment_runs
        $script:I2gState.discovery_budget_inference = if ($launch) { 'LAUNCH_EVIDENCE' } else { 'SPAWN_INTENT_UNCERTAIN_FAIL_CLOSED' }
        Save-I2gState
    }
    if (Test-Path -LiteralPath $errorPath -PathType Leaf) {
        throw ("$Phase service harness failed: " + (Get-Content -LiteralPath $errorPath -Raw))
    }
    if (-not (Test-Path -LiteralPath $discoveryPath -PathType Leaf)) {
        throw "$Phase did not produce durable discovery evidence before timeout."
    }
    $discoveryEvidence = Read-I2gJson -Path $discoveryPath
    if ($Phase -eq 'TREATMENT') {
        $script:I2gState.treatment_discovery_completed = $true
        $script:TreatmentDiscoveryCompleted = $true
        Save-I2gState
    }
    $stop = Stop-I2eService -ServiceName $I2gServiceName
    $serviceAfterStop = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
    $processes = Get-I2gOwnedProcessCounts
    if ($Phase -eq 'TREATMENT') {
        $script:I2gState.treatment_service_phase_completed = $true
        Save-I2gState
    }
    [ordered]@{
        phase = $Phase
        stop = $stop
        service_after_stop = $serviceAfterStop
        processes_after_stop = $processes
        spawn_intent = $spawnIntent
        launch_evidence = $launch
        discovery = $discoveryEvidence
    }
}

function Invoke-I2gRollback {
    $cleanupStarted = $ServiceCreated -or $SystemProfileAddedByRun -or $ProfileSingleAddedByRun
    $evidence = [ordered]@{
        schema = 'amd-i2g-real-rollback/v1'
        qualification_only = $true
        run_id = $RunId
        cleanup_required = $cleanupStarted
        service_stop_attempted = $false
        service_stop_verified = $false
        service_state_after_stop = 'ABSENT'
        service_pid_after_stop = 0
        owned_process_count_after_stop = $null
        process_cleanup = $false
        profile_single_remove_attempted = $false
        profile_single_cleanup = $false
        system_profile_remove_attempted = $false
        system_profile_cleanup = $false
        service_delete_attempted = $false
        service_cleanup = $false
        acl_restore_attempted = $false
        acl_restore = $false
        cleanup_result = 'PASS'
        errors = @()
    }
    if (-not $cleanupStarted) {
        if ($null -ne $RunRoot) {
            Write-I2gAtomicJson -Path (Join-Path $RunRoot 'ROLLBACK.json') -Value $evidence
        }
        return $evidence
    }
    if ($null -ne $script:I2gState -and $null -ne $RunRoot) {
        $script:I2gState.rollback_intent_durable = $true
        Set-I2gState -State 'RollbackRunning'
    }
    try {
        if ($ServiceCreated) {
            $evidence.service_stop_attempted = $true
            $script:I2gState.service_stop_intent_durable = $true
            Save-I2gState
            $stop = Stop-I2eService -ServiceName $I2gServiceName
            $evidence.service_state_after_stop = [string]$stop.state
            $evidence.service_pid_after_stop = [int64]$stop.process_id
        }
        $serviceAfterStop = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
        $evidence.service_stop_verified = (-not $serviceAfterStop.present) -or
            ($serviceAfterStop.state -eq 'Stopped' -and $serviceAfterStop.process_id -eq 0)
        $evidence.service_state_after_stop = if ($serviceAfterStop.present) { $serviceAfterStop.state } else { 'ABSENT' }
        $evidence.service_pid_after_stop = if ($serviceAfterStop.present) { $serviceAfterStop.process_id } else { 0 }
        if (-not $evidence.service_stop_verified) { throw 'Service stop was not verified with PID=0.' }

        $processes = Get-I2gOwnedProcessCounts
        if ($processes.owned_process_count -gt 0) {
            $script:I2gState.child_cleanup_intent_durable = $true
            Save-I2gState
            Stop-I2gOwnedProcesses
            $processes = Get-I2gOwnedProcessCounts
        }
        $evidence.owned_process_count_after_stop = $processes.owned_process_count
        $evidence.process_cleanup = $processes.owned_process_count -eq 0
        if (-not $evidence.process_cleanup) { throw 'Owned AMD/broker process residue remains.' }

        if ($null -ne $ServiceSid) {
            if ($ProfileSingleAddedByRun) {
                $evidence.profile_single_remove_attempted = $true
                $script:I2gState.profile_single_ownership.removal_intent_durable = $true
                Save-I2gState
                $current = Get-I2gRightSnapshot -Label 'rollback-before-profile-single-remove' -Sid $ServiceSid -Right $I2gTreatmentRight
                if ($current.status -ne 'READ') { throw 'ProfileSingle rollback readback unavailable.' }
                if ($current.direct_present -or $current.assignment_present) {
                    Remove-I2gExactRight -Sid $ServiceSid -Right $I2gTreatmentRight
                }
                $after = Get-I2gRightSnapshot -Label 'rollback-after-profile-single-remove' -Sid $ServiceSid -Right $I2gTreatmentRight
                $evidence.profile_single_cleanup = $after.status -eq 'READ' -and -not $after.direct_present -and -not $after.assignment_present
                $script:I2gState.profile_single_ownership.removal_observed_absent = $evidence.profile_single_cleanup
                Save-I2gState
                if (-not $evidence.profile_single_cleanup) { throw 'ProfileSingle rollback was not verified.' }
            } else {
                $evidence.profile_single_cleanup = $true
            }
            if ($SystemProfileAddedByRun) {
                $evidence.system_profile_remove_attempted = $true
                $script:I2gState.system_profile_ownership.removal_intent_durable = $true
                Save-I2gState
                $current = Get-I2gRightSnapshot -Label 'rollback-before-system-profile-remove' -Sid $ServiceSid -Right $I2gControlRight
                if ($current.status -ne 'READ') { throw 'SystemProfile rollback readback unavailable.' }
                if ($current.direct_present -or $current.assignment_present) {
                    Remove-I2gExactRight -Sid $ServiceSid -Right $I2gControlRight
                }
                $after = Get-I2gRightSnapshot -Label 'rollback-after-system-profile-remove' -Sid $ServiceSid -Right $I2gControlRight
                $evidence.system_profile_cleanup = $after.status -eq 'READ' -and -not $after.direct_present -and -not $after.assignment_present
                $script:I2gState.system_profile_ownership.removal_observed_absent = $evidence.system_profile_cleanup
                Save-I2gState
                if (-not $evidence.system_profile_cleanup) { throw 'SystemProfile rollback was not verified.' }
            } else {
                $evidence.system_profile_cleanup = $true
            }
        }
        if ($ServiceCreated) {
            $evidence.service_delete_attempted = $true
            $script:I2gState.service_delete_intent_durable = $true
            Save-I2gState
            Remove-I2eService -ServiceName $I2gServiceName
            $evidence.service_cleanup = -not (Get-I2eServiceSnapshot -ServiceName $I2gServiceName).present
            $script:I2gState.service_absent_after_delete = $evidence.service_cleanup
            Save-I2gState
            if (-not $evidence.service_cleanup) { throw 'Qualification service remains after delete.' }
        } else {
            $evidence.service_cleanup = $true
        }
    }
    catch {
        $evidence.cleanup_result = 'FAIL'
        $evidence.errors += $_.Exception.Message
    }
    if ($AclWasMutated -and $null -ne $AclSddlBefore) {
        $evidence.acl_restore_attempted = $true
        try {
            $acl = Get-Acl -LiteralPath $RunRoot
            $acl.SetSecurityDescriptorSddlForm($AclSddlBefore)
            Set-Acl -LiteralPath $RunRoot -AclObject $acl
            $evidence.acl_restore = $true
        }
        catch {
            $evidence.cleanup_result = 'FAIL'
            $evidence.errors += ('ACL restore failed: ' + $_.Exception.Message)
        }
    } else {
        $evidence.acl_restore = $true
    }
    if ($null -ne $RunRoot) {
        Write-I2gAtomicJson -Path (Join-Path $RunRoot 'ROLLBACK.json') -Value $evidence
    }
    $evidence
}

function Get-I2gStateValue {
    param(
        [Parameter(Mandatory = $true)]$State,
        [Parameter(Mandatory = $true)][string]$Name,
        $Default = $null
    )
    $property = $State.PSObject.Properties[$Name]
    if ($null -eq $property) { return $Default }
    $property.Value
}

function Get-I2gServiceDefinition {
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$I2gServiceName'" -ErrorAction Stop |
        Select-Object -First 1
    if ($null -eq $service) {
        return [ordered]@{
            present = $false
            name = $I2gServiceName
            path_name = $null
            start_name = $null
            state = 'ABSENT'
            process_id = 0
        }
    }
    [ordered]@{
        present = $true
        name = [string]$service.Name
        path_name = [string]$service.PathName
        start_name = [string]$service.StartName
        state = [string]$service.State
        process_id = [int64]$service.ProcessId
    }
}

function Test-I2gExactServiceDefinition {
    param([Parameter(Mandatory = $true)]$Definition)
    if (-not [bool]$Definition.present) { return $false }
    $expectedPath = ('"{0}" --i2g-real-service' -f $ArtifactPath)
    ([string]$Definition.name -ceq $I2gServiceName) -and
        ([string]$Definition.start_name -ieq $I2gServiceAccount) -and
        ([string]$Definition.path_name).Trim() -ceq $expectedPath
}

function Invoke-I2gIncompleteRunRecovery {
    param([Parameter(Mandatory = $true)][string]$RecoveryRoot)

    $stateFiles = @(Get-ChildItem -LiteralPath $RecoveryRoot -Filter 'STATE-*.json' -File |
        Sort-Object Name -Descending)
    if ($stateFiles.Count -eq 0) { throw 'Incomplete I2G run has no persisted state journal.' }
    $state = Read-I2gJson -Path $stateFiles[0].FullName
    if ((Get-I2gStateValue -State $state -Name 'schema') -ne 'amd-i2g-real-persisted-state/v1' -or
        (Get-I2gStateValue -State $state -Name 'service_name') -cne $I2gServiceName -or
        (Get-I2gStateValue -State $state -Name 'service_account') -cne $I2gServiceAccount -or
        (Get-I2gStateValue -State $state -Name 'service_account_sid') -cne $I2gServiceAccountSid) {
        throw 'Persisted I2G recovery journal is outside the fixed service contract.'
    }

    $serviceDefinition = Get-I2gServiceDefinition
    $serviceIntent = [bool](Get-I2gStateValue -State $state -Name 'service_mutation_intent_durable' -Default $false)
    $serviceCreated = [bool](Get-I2gStateValue -State $state -Name 'service_created_by_run' -Default $false)
    $serviceExact = Test-I2gExactServiceDefinition -Definition $serviceDefinition
    if ([bool]$serviceDefinition.present -and -not ($serviceExact -and ($serviceIntent -or $serviceCreated))) {
        throw 'Recovery found a present target service whose ownership cannot be proven.'
    }

    $serviceSid = [string](Get-I2gStateValue -State $state -Name 'service_sid')
    if ([string]::IsNullOrWhiteSpace($serviceSid) -and [bool]$serviceDefinition.present) {
        $serviceSid = Resolve-I2eServiceSid -ServiceSidAccount $I2gServiceSidAccount
    }
    if (-not [string]::IsNullOrWhiteSpace($serviceSid) -and $serviceSid -notmatch '^S-1-5-80-') {
        throw "Recovery journal contains a non-Service SID: $serviceSid"
    }

    $systemOwnership = Get-I2gStateValue -State $state -Name 'system_profile_ownership'
    $profileOwnership = Get-I2gStateValue -State $state -Name 'profile_single_ownership'
    if ($null -eq $systemOwnership -or $null -eq $profileOwnership) {
        throw 'Recovery journal is missing right ownership records.'
    }
    $systemPreexisting = [bool](Get-I2gStateValue -State $systemOwnership -Name 'preexisting' -Default $false)
    $profilePreexisting = [bool](Get-I2gStateValue -State $profileOwnership -Name 'preexisting' -Default $false)
    $systemIntent = [bool](Get-I2gStateValue -State $systemOwnership -Name 'mutation_intent_durable' -Default $false)
    $profileIntent = [bool](Get-I2gStateValue -State $profileOwnership -Name 'mutation_intent_durable' -Default $false)
    if (($systemIntent -or $profileIntent) -and [string]::IsNullOrWhiteSpace($serviceSid)) {
        throw 'Recovery cannot safely reconcile a right mutation without the persisted Service SID.'
    }

    $recoveryEvidence = [ordered]@{
        schema = 'amd-i2g-real-recovery-reconciliation/v1'
        qualification_only = $true
        run_id = [string](Get-I2gStateValue -State $state -Name 'run_id')
        persisted_state = $state.state
        recovery_only = $true
        discovery_relaunched = $false
        retry_occurred = $false
        service_ownership_inferred = $serviceExact -and ($serviceIntent -or $serviceCreated)
        service_stop_verified = $false
        process_cleanup = $false
        system_profile_cleanup = $false
        profile_single_cleanup = $false
        service_cleanup = $false
        acl_restore = $false
        active_config_cleanup = $false
        errors = @()
    }

    $state.state = 'RollbackRunning'
    $state.recovery_required = $true
    $state.recovery_mode = 'MACHINE_OBSERVATION_ONLY_NO_DISCOVERY'
    Write-I2gAtomicJson -Path (Join-Path $RecoveryRoot 'STATE-RECOVERY-0001.json') -Value $state

    try {
        if ([bool]$serviceDefinition.present) {
            $stop = Stop-I2eService -ServiceName $I2gServiceName
            $afterStop = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
            $recoveryEvidence.service_stop_verified = (-not $afterStop.present) -or
                ($afterStop.state -eq 'Stopped' -and [int64]$afterStop.process_id -eq 0)
            if (-not $recoveryEvidence.service_stop_verified) { throw 'Recovery could not verify service stop/PID=0.' }
        } else {
            $recoveryEvidence.service_stop_verified = $true
        }

        $processes = Get-I2gOwnedProcessCounts
        if ([int64]$processes.owned_process_count -gt 0) {
            Stop-I2gOwnedProcesses
            $processes = Get-I2gOwnedProcessCounts
        }
        $recoveryEvidence.process_cleanup = [int64]$processes.owned_process_count -eq 0
        if (-not $recoveryEvidence.process_cleanup) { throw 'Recovery found exact owned process residue after cleanup.' }

        if (-not [string]::IsNullOrWhiteSpace($serviceSid)) {
            if ($profileIntent -and -not $profilePreexisting) {
                $profile = Get-I2gRightSnapshot -Label 'recovery-before-profile-single-remove' -Sid $serviceSid -Right $I2gTreatmentRight
                if ($profile.status -ne 'READ') { throw 'Recovery ProfileSingle readback unavailable.' }
                if ($profile.direct_present -or $profile.assignment_present) {
                    Remove-I2gExactRight -Sid $serviceSid -Right $I2gTreatmentRight
                }
                $profileAfter = Get-I2gRightSnapshot -Label 'recovery-after-profile-single-remove' -Sid $serviceSid -Right $I2gTreatmentRight
                $recoveryEvidence.profile_single_cleanup = $profileAfter.status -eq 'READ' -and
                    -not $profileAfter.direct_present -and -not $profileAfter.assignment_present
            } else {
                $recoveryEvidence.profile_single_cleanup = $true
            }
            if ($systemIntent -and -not $systemPreexisting) {
                $system = Get-I2gRightSnapshot -Label 'recovery-before-system-profile-remove' -Sid $serviceSid -Right $I2gControlRight
                if ($system.status -ne 'READ') { throw 'Recovery SystemProfile readback unavailable.' }
                if ($system.direct_present -or $system.assignment_present) {
                    Remove-I2gExactRight -Sid $serviceSid -Right $I2gControlRight
                }
                $systemAfter = Get-I2gRightSnapshot -Label 'recovery-after-system-profile-remove' -Sid $serviceSid -Right $I2gControlRight
                $recoveryEvidence.system_profile_cleanup = $systemAfter.status -eq 'READ' -and
                    -not $systemAfter.direct_present -and -not $systemAfter.assignment_present
            } else {
                $recoveryEvidence.system_profile_cleanup = $true
            }
        } else {
            $recoveryEvidence.system_profile_cleanup = -not $systemIntent
            $recoveryEvidence.profile_single_cleanup = -not $profileIntent
        }

        if ($serviceExact -and ($serviceIntent -or $serviceCreated)) {
            Remove-I2eService -ServiceName $I2gServiceName
        }
        $recoveryEvidence.service_cleanup = -not (Get-I2eServiceSnapshot -ServiceName $I2gServiceName).present

        $aclSddlBefore = [string](Get-I2gStateValue -State $state -Name 'run_root_acl_sddl_before')
        $aclIntent = [bool](Get-I2gStateValue -State $state -Name 'acl_mutation_intent_durable' -Default $false)
        if ($aclIntent) {
            if ([string]::IsNullOrWhiteSpace($aclSddlBefore)) { throw 'Recovery journal is missing the pre-mutation ACL snapshot.' }
            $acl = Get-Acl -LiteralPath $RecoveryRoot
            $acl.SetSecurityDescriptorSddlForm($aclSddlBefore)
            Set-Acl -LiteralPath $RecoveryRoot -AclObject $acl
        }
        $recoveryEvidence.acl_restore = $true

        if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) {
            $config = Read-I2gJson -Path $ConfigPath
            if ([string]$config.service_name -cne $I2gServiceName -or
                ([IO.Path]::GetFullPath([string]$config.output_root) -cne [IO.Path]::GetFullPath($RecoveryRoot))) {
                throw 'Recovery found an active config that does not belong to this run.'
            }
            Remove-Item -LiteralPath $ConfigPath -Force
        }
        $recoveryEvidence.active_config_cleanup = -not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)
        if (-not $recoveryEvidence.active_config_cleanup) { throw 'Recovery active config cleanup was not verified.' }
    }
    catch {
        $recoveryEvidence.errors += $_.Exception.Message
    }

    $finalService = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
    $finalProcesses = Get-I2gOwnedProcessCounts
    $finalSystem = if (-not [string]::IsNullOrWhiteSpace($serviceSid)) { Get-I2gRightSnapshot -Label 'recovery-final-system-profile' -Sid $serviceSid -Right $I2gControlRight } else { $null }
    $finalProfile = if (-not [string]::IsNullOrWhiteSpace($serviceSid)) { Get-I2gRightSnapshot -Label 'recovery-final-profile-single' -Sid $serviceSid -Right $I2gTreatmentRight } else { $null }
    $finalMachineState = [ordered]@{
        service_present = $finalService.present
        service_state = $finalService.state
        service_pid = $finalService.process_id
        owned_process_count = $finalProcesses.owned_process_count
        system_profile_right = $finalSystem
        profile_single_right = $finalProfile
        active_config_present = Test-Path -LiteralPath $ConfigPath -PathType Leaf
    }
    $systemFinalSafe = if ($systemPreexisting) {
        $null -ne $finalSystem -and $finalSystem.status -eq 'READ'
    } else {
        $null -eq $finalSystem -or ($finalSystem.status -eq 'READ' -and
            -not $finalSystem.direct_present -and -not $finalSystem.assignment_present)
    }
    $profileFinalSafe = if ($profilePreexisting) {
        $null -ne $finalProfile -and $finalProfile.status -eq 'READ'
    } else {
        $null -eq $finalProfile -or ($finalProfile.status -eq 'READ' -and
            -not $finalProfile.direct_present -and -not $finalProfile.assignment_present)
    }
    $cleanupPass = $recoveryEvidence.errors.Count -eq 0 -and
        -not [bool]$finalMachineState.service_present -and
        [int64]$finalMachineState.service_pid -eq 0 -and
        [int64]$finalMachineState.owned_process_count -eq 0 -and
        $recoveryEvidence.service_stop_verified -and
        $recoveryEvidence.process_cleanup -and
        $recoveryEvidence.system_profile_cleanup -and
        $recoveryEvidence.profile_single_cleanup -and
        $recoveryEvidence.service_cleanup -and
        $recoveryEvidence.acl_restore -and
        $recoveryEvidence.active_config_cleanup -and
        $systemFinalSafe -and $profileFinalSafe
    $recoveryEvidence.cleanup_result = if ($cleanupPass) { 'PASS' } else { 'FAIL' }

    $rollbackPath = Join-Path $RecoveryRoot 'ROLLBACK.json'
    if (Test-Path -LiteralPath $rollbackPath -PathType Leaf) { $rollbackPath = Join-Path $RecoveryRoot 'ROLLBACK-RECOVERY.json' }
    Write-I2gAtomicJson -Path $rollbackPath -Value $recoveryEvidence
    $state.state = if ($cleanupPass) { 'RollbackComplete' } else { 'Failed' }
    $state.recovery_required = -not $cleanupPass
    Write-I2gAtomicJson -Path (Join-Path $RecoveryRoot 'STATE-RECOVERY-0002.json') -Value $state

    $controlRuns = if ((Test-Path -LiteralPath (Join-Path $RecoveryRoot 'CONTROL-DISCOVERY-SPAWN-INTENT.json') -PathType Leaf) -or
        (Test-Path -LiteralPath (Join-Path $RecoveryRoot 'CONTROL-AMD-DISCOVERY-LAUNCH.json') -PathType Leaf)) { 1 } else { 0 }
    $treatmentRuns = if ((Test-Path -LiteralPath (Join-Path $RecoveryRoot 'TREATMENT-DISCOVERY-SPAWN-INTENT.json') -PathType Leaf) -or
        (Test-Path -LiteralPath (Join-Path $RecoveryRoot 'TREATMENT-AMD-DISCOVERY-LAUNCH.json') -PathType Leaf)) { 1 } else { 0 }
    $summary = [ordered]@{
        schema = 'amd-i2g-real-final-summary/v1'
        result = if ($cleanupPass) { 'INVALID_NO_CAUSAL_INTERPRETATION' } else { 'BLOCKED' }
        task_id = 'AMD-PRIVILEGE-I2G-REAL-QUALIFICATION'
        run_id = [string](Get-I2gStateValue -State $state -Name 'run_id')
        recovery_only = $true
        human_real_run_authorization = 'CONSUMED'
        real_privileged_run = if ($controlRuns + $treatmentRuns -gt 0) { 'YES' } else { 'NO' }
        real_cleanup_run = 'YES'
        control_runs = $controlRuns
        treatment_runs = $treatmentRuns
        total_discovery_runs = $controlRuns + $treatmentRuns
        power_sampling_runs = 0
        retry_occurred = $false
        causal_interpretation_valid = $false
        treatment_allowed = $false
        preexisting_system_profile_right = $systemPreexisting
        preexisting_profile_single_right = $profilePreexisting
        system_profile_added_by_run = $systemIntent -and -not $systemPreexisting
        profile_single_added_by_run = $profileIntent -and -not $profilePreexisting
        system_profile_cleanup = $recoveryEvidence.system_profile_cleanup
        profile_single_cleanup = $recoveryEvidence.profile_single_cleanup
        service_cleanup = $recoveryEvidence.service_cleanup
        process_cleanup = $recoveryEvidence.process_cleanup
        final_machine_state = $finalMachineState
        evidence_root = $RecoveryRoot
        primary_error = 'Recovery path executed; no scientific result is admissible after crash/restart.'
        cleanup_error = if ($recoveryEvidence.errors.Count -gt 0) { $recoveryEvidence.errors -join '; ' } else { $null }
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    $finalPath = Join-Path $RecoveryRoot 'FINAL-SUMMARY.json'
    if (Test-Path -LiteralPath $finalPath -PathType Leaf) { $finalPath = Join-Path $RecoveryRoot 'FINAL-SUMMARY-RECOVERY.json' }
    Write-I2gAtomicJson -Path $finalPath -Value $summary
    $summary
}

$recoveryCandidates = @()
if (Test-Path -LiteralPath $QualificationRoot -PathType Container) {
    $recoveryCandidates = @(Get-ChildItem -LiteralPath $QualificationRoot -Directory -ErrorAction Stop | Where-Object {
        (Test-Path -LiteralPath (Join-Path $_.FullName 'STATE-0000.json') -PathType Leaf) -and
        -not (Test-Path -LiteralPath (Join-Path $_.FullName 'FINAL-SUMMARY.json') -PathType Leaf) -and
        -not (Test-Path -LiteralPath (Join-Path $_.FullName 'FINAL-SUMMARY-RECOVERY.json') -PathType Leaf)
    })
}
if ($recoveryCandidates.Count -gt 0) {
    try {
        $null = Assert-I2eAdministrator
        if ($recoveryCandidates.Count -ne 1) { throw 'Multiple incomplete I2G runs exist; refusing ambiguous recovery.' }
        $recovered = Invoke-I2gIncompleteRunRecovery -RecoveryRoot $recoveryCandidates[0].FullName
        $recovered | ConvertTo-Json -Depth 50
        if ([string]$recovered.result -eq 'BLOCKED') { exit 1 }
        exit 0
    }
    catch {
        [ordered]@{
            result = 'BLOCKED'
            task_id = 'AMD-PRIVILEGE-I2G-REAL-QUALIFICATION'
            recovery_only = $true
            human_real_run_authorization = 'CONSUMED'
            real_privileged_run = 'UNKNOWN'
            real_cleanup_run = 'ATTEMPTED'
            causal_interpretation_valid = $false
            retry_occurred = $false
            primary_error = $_.Exception.Message
            evidence_root = if ($recoveryCandidates.Count -eq 1) { $recoveryCandidates[0].FullName } else { $null }
        } | ConvertTo-Json -Depth 20
        exit 1
    }
}

try {
    $null = Assert-I2eAdministrator
    $HarnessIdentity = Test-I2gHarnessArtifactIdentity -Path $ArtifactPath
    if (-not $HarnessIdentity.pass) { throw "I2G harness artifact identity rejected: $($HarnessIdentity.reason)" }
    $AmdCliIdentity = Get-I2gAmdCliIdentity
    if (-not $AmdCliIdentity.identity_pass) { throw 'AMD CLI identity gate failed before mutation.' }

    New-Item -ItemType Directory -Force -Path $QualificationRoot | Out-Null
    if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) {
        throw "Active I2G config exists; recovery/ownership state must be audited before a new run: $ConfigPath"
    }
    $incomplete = @(Get-ChildItem -LiteralPath $QualificationRoot -Directory -ErrorAction SilentlyContinue | Where-Object {
        (Test-Path -LiteralPath (Join-Path $_.FullName 'STATE-0000.json') -PathType Leaf) -and
        -not (Test-Path -LiteralPath (Join-Path $_.FullName 'FINAL-SUMMARY.json') -PathType Leaf)
    })
    if ($incomplete.Count -gt 0) { throw 'An incomplete prior I2G run directory exists; refusing a new experiment.' }

    $serviceBefore = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
    $ownedBefore = Get-I2gOwnedProcessCounts
    if ($serviceBefore.present -or $ownedBefore.owned_process_count -ne 0) {
        throw 'Unknown/pre-existing target service or owned-path process detected; refusing takeover.'
    }
    $RunId = [Guid]::NewGuid().ToString('N')
    $RunRoot = Join-Path $QualificationRoot $RunId
    New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null
    $AclSddlBefore = (Get-Acl -LiteralPath $RunRoot).Sddl

    $script:I2gState = [ordered]@{
        schema = 'amd-i2g-real-persisted-state/v1'
        qualification_only = $true
        run_id = $RunId
        state = 'Prepared'
        service_name = $I2gServiceName
        service_account = $I2gServiceAccount
        service_account_sid = $I2gServiceAccountSid
        service_sid = $null
        service_created_by_run = $false
        system_profile_ownership = [ordered]@{ preexisting = $false; mutation_intent_durable = $false; observed_present = $false; owned_by_run = $false; removal_intent_durable = $false; removal_observed_absent = $false }
        profile_single_ownership = [ordered]@{ preexisting = $false; mutation_intent_durable = $false; observed_present = $false; owned_by_run = $false; removal_intent_durable = $false; removal_observed_absent = $false }
        control_discovery_spawn_intent_durable = $false
        treatment_discovery_spawn_intent_durable = $false
        control_runs = 0
        treatment_runs = 0
        total_runs = 0
        power_sampling_runs = 0
        retry_occurred = $false
        treatment_allowed = $false
        treatment_allowed_by_scientific_gate = $false
        treatment_policy_mutation_started = $false
        treatment_policy_mutation_completed = $false
        treatment_service_phase_started = $false
        treatment_service_phase_completed = $false
        treatment_discovery_started = $false
        treatment_discovery_completed = $false
        harness_runtime_failure = $false
        failure_class = 'NONE'
        scientific_result = 'NOT_OBTAINED'
        recovery_required = $false
        active_config_path = $ConfigPath
    }
    Save-I2gState

    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'EXPERIMENT-MANIFEST.json') -Value ([ordered]@{
        schema = 'amd-i2g-real-experiment-manifest/v1'
        qualification_only = $true
        run_id = $RunId
        variable = $I2gTreatmentRight
        experiment_shape = 'PAIRED_CONTROL_TREATMENT'
        service_name = $I2gServiceName
        service_account = $I2gServiceAccount
        service_account_sid = $I2gServiceAccountSid
        service_sid_account = $I2gServiceSidAccount
        fixed_operation = 'AMDuProf counter discovery only'
        fixed_cli_arguments = @('timechart', '--list')
        max_control_runs = $I2gMaxControlRuns
        max_treatment_runs = $I2gMaxTreatmentRuns
        max_total_runs = $I2gMaxTotalRuns
        power_sampling_runs = 0
        harness_artifact = $HarnessIdentity
        amd_cli_identity = $AmdCliIdentity
        authorization = 'GRANTED_FOR_THIS_TASK_ONLY'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'PRE-MACHINE-STATE.json') -Value ([ordered]@{
        schema = 'amd-i2g-pre-machine-state/v1'
        qualification_only = $true
        service = $serviceBefore
        service_sid_account = $I2gServiceSidAccount
        service_sid_resolution_before_service_create = 'UNAVAILABLE: Windows virtual Service SID is materialized only after exact service registration.'
        owned_processes_before = $ownedBefore
        artifact_identity = $HarnessIdentity
        amd_cli_identity = $AmdCliIdentity
        preexisting_system_profile_right = 'DEFERRED_UNTIL_SERVICE_SID_MATERIALIZATION'
        preexisting_profile_single_right = 'DEFERRED_UNTIL_SERVICE_SID_MATERIALIZATION'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })

    Set-I2gState -State 'Prepared'
    $script:I2gState.service_mutation_intent_durable = $true
    Set-I2gState -State 'Prepared'
    $binPath = '"{0}" --i2g-real-service' -f $ArtifactPath
    $createArgs = New-QualificationServiceCreateArguments -ServiceName $I2gServiceName -BinPath $binPath -ServiceAccount $I2gServiceAccount -DisplayName 'Resource Timeline AMD I2G paired qualification'
    Invoke-I2eSc -Arguments $createArgs | Out-Null
    $ServiceCreated = $true
    $script:I2gState.service_created_by_run = $true
    Set-I2gState -State 'ControlPolicyReady'
    Invoke-I2eSc -Arguments @('sidtype', $I2gServiceName, 'unrestricted') | Out-Null
    Assert-I2eServiceSidType -ServiceName $I2gServiceName
    $ServiceSid = Resolve-I2eServiceSid -ServiceSidAccount $I2gServiceSidAccount
    $script:I2gState.service_sid = $ServiceSid
    Save-I2gState
    $script:I2gState.run_root_acl_sddl_before = $AclSddlBefore
    $script:I2gState.acl_mutation_intent_durable = $true
    Save-I2gState
    Set-I2eDirectoryAcl -Path $RunRoot -ServiceSid $ServiceSid
    $AclWasMutated = $true
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'I2G-ACL.json') -Value ([ordered]@{ schema = 'amd-i2g-acl/v1'; service_sid = $ServiceSid; restored_after_cleanup = $false })

    $systemBefore = Get-I2gRightSnapshot -Label 'pre-policy-system-profile' -Sid $ServiceSid -Right $I2gControlRight
    $profileBefore = Get-I2gRightSnapshot -Label 'pre-policy-profile-single' -Sid $ServiceSid -Right $I2gTreatmentRight
    if ($systemBefore.status -ne 'READ' -or $profileBefore.status -ne 'READ') { throw 'Pre-policy LSA readback unavailable.' }
    $SystemProfilePreexisting = [bool]($systemBefore.direct_present -or $systemBefore.assignment_present)
    $ProfileSinglePreexisting = [bool]($profileBefore.direct_present -or $profileBefore.assignment_present)
    $ControlPolicyBefore = $systemBefore
    $TreatmentPolicyBefore = $profileBefore
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'POST-SERVICE-IDENTITY-PRE-POLICY.json') -Value ([ordered]@{
        schema = 'amd-i2g-post-service-identity-pre-policy/v1'
        service_sid = $ServiceSid
        system_profile = $systemBefore
        profile_single = $profileBefore
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    if ($ProfileSinglePreexisting) {
        $PrimaryError = 'INVALID_NO_CAUSAL_INTERPRETATION: SeProfileSingleProcessPrivilege pre-existed on target Service SID.'
        throw $PrimaryError
    }
    $allowedRights = @($I2gControlRight, $I2gTreatmentRight)
    $observedDirectRights = @($systemBefore.direct_rights) + @($profileBefore.direct_rights)
    $observedDirectRights = @($observedDirectRights | Sort-Object -Unique)
    $unexpectedRights = @($observedDirectRights | Where-Object { $_ -notin $allowedRights })
    if ($unexpectedRights.Count -gt 0) { throw ('Unknown pre-existing Service SID rights: ' + ($unexpectedRights -join ', ')) }

    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'CONTROL-POLICY.json') -Value ([ordered]@{
        schema = 'amd-i2g-control-policy/v1'
        service_sid = $ServiceSid
        direct_rights_before = @($systemBefore.direct_rights)
        intended_direct_rights = @($I2gControlRight)
        preexisting_system_profile_right = $SystemProfilePreexisting
        policy_delta = if ($SystemProfilePreexisting) { 'NONE_PREEXISTING_PRESERVED' } else { 'ADD_SeSystemProfilePrivilege' }
    })
    $script:I2gState.system_profile_ownership.preexisting = $SystemProfilePreexisting
    $script:I2gState.system_profile_ownership.mutation_intent_durable = -not $SystemProfilePreexisting
    Save-I2gState
    if (-not $SystemProfilePreexisting) {
        Add-I2gExactRight -Sid $ServiceSid -Right $I2gControlRight
        $SystemProfileAddedByRun = $true
    }
    $systemAfter = Get-I2gRightSnapshot -Label 'control-policy-after-add' -Sid $ServiceSid -Right $I2gControlRight
    if ($systemAfter.status -ne 'READ' -or -not $systemAfter.direct_present -or -not $systemAfter.assignment_present) { throw 'CONTROL SystemProfile right readback failed.' }
    $script:I2gState.system_profile_ownership.observed_present = $true
    $script:I2gState.system_profile_ownership.owned_by_run = $SystemProfileAddedByRun
    $script:I2gState.system_profile_right_added_by_run = $SystemProfileAddedByRun
    Save-I2gState
    if (-not (Test-I2gExactPolicy -Snapshot $systemAfter -ExpectedRights @($I2gControlRight))) { throw 'CONTROL direct Service SID policy is not exact.' }
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'CONTROL-POLICY-AFTER.json') -Value $systemAfter

    $controlConfig = [ordered]@{
        schema = 'amd-i2g-real-service-config/v1'
        qualification_only = $true
        service_name = $I2gServiceName
        service_account = $I2gServiceAccount
        service_account_sid = $I2gServiceAccountSid
        service_sid = $ServiceSid
        scope = $RunId
        output_root = $RunRoot
        phase = 'CONTROL'
        expected_profile_single_process_privilege = $false
        expected_amd_cli_path = $I2gFixedAmdCliPath
        expected_amd_cli_sha256 = $I2gFixedAmdCliSha256
        expected_amd_cli_version = $I2gFixedAmdCliVersion
        expected_amd_cli_architecture = $I2gFixedAmdCliArchitecture
        harness_artifact_sha256 = $HarnessIdentity.sha256
    }
    $controlPhase = Invoke-I2gServicePhase -Phase CONTROL -Config $controlConfig
    $ControlResult = [string]$controlPhase.discovery.availability
    $controlPreToken = Read-I2gJson -Path (Join-Path $RunRoot 'CONTROL-TOKEN-PRE.json')
    $controlPostToken = Read-I2gJson -Path (Join-Path $RunRoot 'CONTROL-TOKEN-POST.json')
    $ControlTokenGate = [bool]$controlPreToken.gate.gate_pass -and [bool]$controlPostToken.gate.gate_pass
    $controlServiceClean = $controlPhase.service_after_stop.state -eq 'Stopped' -and [int64]$controlPhase.service_after_stop.process_id -eq 0
    $controlProcessClean = [int64]$controlPhase.processes_after_stop.owned_process_count -eq 0
    $ControlTeardownPass = $controlServiceClean -and $controlProcessClean
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'CONTROL-TEARDOWN.json') -Value ([ordered]@{
        schema = 'amd-i2g-control-teardown/v1'
        service_stopped = $controlServiceClean
        service_pid_zero = ([int64]$controlPhase.service_after_stop.process_id -eq 0)
        token_absent_after_stop = $controlServiceClean
        token_absence_basis = 'service PID=0 after stop; no process token remains observable'
        token_gone = $controlServiceClean
        amd_child_absent = ([int64]$controlPhase.processes_after_stop.amd_cli_process_count -eq 0)
        owned_process_count = $controlPhase.processes_after_stop.owned_process_count
        owned_process_tree_empty = $controlProcessClean
        exact_child_identity_absent = ($controlPhase.processes_after_stop.amd_cli_process_count -eq 0)
        child_was_owned = [bool]$controlPhase.launch_evidence
        pass = $ControlTeardownPass
    })
    $controlScientificGate = $ControlResult -ceq 'POWER_UNAVAILABLE' -and $ControlTokenGate -and $ControlTeardownPass
    if (-not $controlScientificGate) {
        $TreatmentAllowed = $false
        $TreatmentAllowedByScientificGate = $false
        $FailureClass = 'SCIENTIFIC_INVALIDATION'
        $ScientificResult = 'NOT_OBTAINED'
        $script:I2gState.treatment_allowed = $false
        $script:I2gState.treatment_allowed_by_scientific_gate = $false
        $script:I2gState.failure_class = $FailureClass
        $script:I2gState.scientific_result = $ScientificResult
        Set-I2gState -State 'Failed'
        throw "CONTROL_DRIFT: CONTROL did not produce a valid stable POWER_UNAVAILABLE baseline: $ControlResult"
    }

    $TreatmentAllowedByScientificGate = $true
    $script:I2gState.treatment_allowed = $true
    $script:I2gState.treatment_allowed_by_scientific_gate = $true
    $TreatmentAllowed = $true
    Set-I2gState -State 'TreatmentPolicyReady'
    $profileBeforeTreatment = Get-I2gRightSnapshot -Label 'treatment-policy-before-add' -Sid $ServiceSid -Right $I2gTreatmentRight
    if ($profileBeforeTreatment.status -ne 'READ' -or $profileBeforeTreatment.direct_present -or $profileBeforeTreatment.assignment_present) {
        throw 'INVALID_CONFIGURATION_DELTA: TREATMENT ProfileSingle right was not absent immediately before mutation.'
    }
    $TreatmentPolicyMutationStarted = $true
    $script:I2gState.treatment_policy_mutation_started = $true
    Save-I2gState
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'TREATMENT-POLICY.json') -Value ([ordered]@{
        schema = 'amd-i2g-treatment-policy/v1'
        service_sid = $ServiceSid
        control_direct_rights = @($systemAfter.direct_rights)
        treatment_intended_direct_rights = @($I2gControlRight, $I2gTreatmentRight)
        allowed_delta = $I2gTreatmentRight
        preexisting_profile_single_right = $false
    })
    $script:I2gState.profile_single_ownership.mutation_intent_durable = $true
    Save-I2gState
    Add-I2gExactRight -Sid $ServiceSid -Right $I2gTreatmentRight
    $ProfileSingleAddedByRun = $true
    $profileAfter = Get-I2gRightSnapshot -Label 'treatment-policy-after-add' -Sid $ServiceSid -Right $I2gTreatmentRight
    if ($profileAfter.status -ne 'READ' -or -not $profileAfter.direct_present -or -not $profileAfter.assignment_present) { throw 'INVALID_CONFIGURATION_DELTA: TREATMENT ProfileSingle right readback failed.' }
    $systemAfterTreatment = Get-I2gRightSnapshot -Label 'treatment-policy-system-profile-after-add' -Sid $ServiceSid -Right $I2gControlRight
    if ($systemAfterTreatment.status -ne 'READ' -or -not $systemAfterTreatment.direct_present -or -not $systemAfterTreatment.assignment_present) {
        throw 'INVALID_CONFIGURATION_DELTA: TREATMENT SystemProfile right readback failed.'
    }
    $script:I2gState.profile_single_ownership.observed_present = $true
    $script:I2gState.profile_single_ownership.owned_by_run = $true
    $script:I2gState.profile_single_right_added_by_run = $true
    Save-I2gState
    if (-not (Test-I2gExactPolicy -Snapshot $profileAfter -ExpectedRights @($I2gControlRight, $I2gTreatmentRight))) {
        throw 'INVALID_CONFIGURATION_DELTA: TREATMENT direct Service SID policy is not exact.'
    }
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'TREATMENT-POLICY-AFTER.json') -Value ([ordered]@{
        schema = 'amd-i2g-treatment-policy-after/v1'
        service_sid = $ServiceSid
        system_profile = $systemAfterTreatment
        profile_single = $profileAfter
        direct_rights = @($profileAfter.direct_rights)
    })
    $controlDirectRights = @($systemAfter.direct_rights | Sort-Object -Unique)
    $treatmentDirectRights = @($profileAfter.direct_rights | Sort-Object -Unique)
    $expectedTreatmentRights = @($I2gControlRight, $I2gTreatmentRight) | Sort-Object -Unique
    $pairedConfigPass = (@($controlDirectRights) -join '|') -ceq $I2gControlRight -and
        (@($treatmentDirectRights) -join '|') -ceq (@($expectedTreatmentRights) -join '|')
    $PairedConfigPass = $pairedConfigPass
    if (-not $PairedConfigPass) { throw 'INVALID_CONFIGURATION_DELTA: CONTROL/TREATMENT configuration delta is not exact.' }
    $TreatmentPolicyMutationCompleted = $true
    $script:I2gState.treatment_policy_mutation_completed = $true
    Save-I2gState
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'PAIRED-CONFIG-COMPARISON.json') -Value ([ordered]@{
        schema = 'amd-i2g-paired-config-comparison/v1'
        allowed_delta = $I2gTreatmentRight
        control_direct_rights = $controlDirectRights
        treatment_direct_rights = $treatmentDirectRights
        pass = $pairedConfigPass
    })

    $treatmentConfig = New-I2gTreatmentConfig -ControlConfig $controlConfig
    $TreatmentServicePhaseStarted = $true
    $script:I2gState.treatment_service_phase_started = $true
    Save-I2gState
    $treatmentPhase = Invoke-I2gServicePhase -Phase TREATMENT -Config $treatmentConfig
    $TreatmentResult = [string]$treatmentPhase.discovery.availability
    $treatmentPreToken = Read-I2gJson -Path (Join-Path $RunRoot 'TREATMENT-TOKEN-PRE.json')
    $treatmentSystemToken = Read-I2gJson -Path (Join-Path $RunRoot 'TREATMENT-TOKEN-SYSTEMPROFILE.json')
    $treatmentFinalToken = Read-I2gJson -Path (Join-Path $RunRoot 'TREATMENT-TOKEN-FINAL.json')
    $TreatmentTokenGate = [bool]$treatmentPreToken.gate.gate_pass -and
        [bool]$treatmentSystemToken.gate.gate_pass -and [bool]$treatmentFinalToken.gate.gate_pass
    $tokenComparison = Compare-I2gTokenPair -Control $controlPostToken -Treatment $treatmentFinalToken
    $TokenComparison = $tokenComparison
    $PairedTokenPass = [bool]$tokenComparison.pass
    Write-I2gAtomicJson -Path (Join-Path $RunRoot 'PAIRED-TOKEN-COMPARISON.json') -Value $tokenComparison
    $controlResultIsValid = $ControlResult -ceq 'POWER_UNAVAILABLE'
    $treatmentResultIsValid = $TreatmentResult -in @('POWER_AVAILABLE', 'POWER_UNAVAILABLE')
    if ($controlResultIsValid -and $treatmentResultIsValid -and $ControlTokenGate -and $TreatmentTokenGate -and [bool]$tokenComparison.pass) {
        if ($TreatmentResult -ceq 'POWER_AVAILABLE') {
            $PairedResult = 'PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT'
        } else {
            $PairedResult = 'PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT'
        }
        $CausalInterpretationValid = $true
        $ScientificResult = $PairedResult
        $FailureClass = 'NONE'
    }
    $script:I2gState.treatment_allowed_by_scientific_gate = $TreatmentAllowedByScientificGate
    $script:I2gState.treatment_policy_mutation_started = $TreatmentPolicyMutationStarted
    $script:I2gState.treatment_policy_mutation_completed = $TreatmentPolicyMutationCompleted
    $script:I2gState.treatment_service_phase_started = $TreatmentServicePhaseStarted
    $script:I2gState.treatment_discovery_spawn_intent_durable = $TreatmentDiscoverySpawnIntentDurable
    $script:I2gState.treatment_discovery_started = $TreatmentDiscoveryStarted
    $script:I2gState.treatment_discovery_completed = $TreatmentDiscoveryCompleted
    $script:I2gState.failure_class = $FailureClass
    $script:I2gState.scientific_result = $ScientificResult
    Set-I2gState -State 'TreatmentComplete'
}
catch {
    if ($null -eq $PrimaryError) { $PrimaryError = $_.Exception.Message }
    if ($FailureClass -eq 'NONE') {
        $FailureClass = Resolve-I2gFailureClass -ErrorMessage ([string]$PrimaryError) `
            -TreatmentAllowedByScientificGate $TreatmentAllowedByScientificGate `
            -ControlRuns ([int]$ControlRuns) -ControlResult ([string]$ControlResult)
    }
    $HarnessRuntimeFailure = $FailureClass -eq 'HARNESS_RUNTIME_ERROR'
    $ScientificResult = if ($HarnessRuntimeFailure) { 'NOT_OBTAINED' } else { 'INVALID_NO_CAUSAL_INTERPRETATION' }
    if ($null -ne $script:I2gState -and $null -ne $RunRoot) {
        $script:I2gState.recovery_required = $ServiceCreated -or $SystemProfileAddedByRun -or $ProfileSingleAddedByRun
        $script:I2gState.treatment_allowed = $TreatmentAllowedByScientificGate
        $script:I2gState.treatment_allowed_by_scientific_gate = $TreatmentAllowedByScientificGate
        $script:I2gState.treatment_policy_mutation_started = $TreatmentPolicyMutationStarted
        $script:I2gState.treatment_policy_mutation_completed = $TreatmentPolicyMutationCompleted
        $script:I2gState.treatment_service_phase_started = $TreatmentServicePhaseStarted
        $script:I2gState.treatment_discovery_spawn_intent_durable = $TreatmentDiscoverySpawnIntentDurable
        $script:I2gState.treatment_discovery_started = $TreatmentDiscoveryStarted
        $script:I2gState.treatment_discovery_completed = $TreatmentDiscoveryCompleted
        $script:I2gState.harness_runtime_failure = $HarnessRuntimeFailure
        $script:I2gState.failure_class = $FailureClass
        $script:I2gState.scientific_result = $ScientificResult
        Set-I2gState -State 'Failed'
    }
}
finally {
    try {
        $Rollback = Invoke-I2gRollback
    }
    catch {
        $CleanupError = $_.Exception.Message
    }
    if ($null -ne $Rollback -and $Rollback.cleanup_result -ne 'PASS') {
        $CleanupError = ($Rollback.errors -join '; ')
    }
    if ($null -ne $RunRoot) {
        $finalService = Get-I2eServiceSnapshot -ServiceName $I2gServiceName
        $finalProcesses = Get-I2gOwnedProcessCounts
        $finalSystem = if ($null -ne $ServiceSid) { Get-I2gRightSnapshot -Label 'final-system-profile' -Sid $ServiceSid -Right $I2gControlRight } else { $null }
        $finalProfile = if ($null -ne $ServiceSid) { Get-I2gRightSnapshot -Label 'final-profile-single' -Sid $ServiceSid -Right $I2gTreatmentRight } else { $null }
        $FinalMachineState = [ordered]@{
            service_present = $finalService.present
            service_state = $finalService.state
            service_pid = $finalService.process_id
            owned_process_count = $finalProcesses.owned_process_count
            amd_cli_process_count = $finalProcesses.amd_cli_process_count
            system_profile_right = $finalSystem
            profile_single_right = $finalProfile
            active_config_present = Test-Path -LiteralPath $ConfigPath -PathType Leaf
        }
        $systemFinalSafe = if ($SystemProfilePreexisting) {
            $null -ne $finalSystem -and $finalSystem.status -eq 'READ'
        } else {
            $null -eq $finalSystem -or ($finalSystem.status -eq 'READ' -and
                -not $finalSystem.direct_present -and -not $finalSystem.assignment_present)
        }
        $profileFinalSafe = if ($ProfileSinglePreexisting) {
            $null -ne $finalProfile -and $finalProfile.status -eq 'READ'
        } else {
            $null -eq $finalProfile -or ($finalProfile.status -eq 'READ' -and
                -not $finalProfile.direct_present -and -not $finalProfile.assignment_present)
        }
        $cleanupPass = $null -ne $Rollback -and $Rollback.cleanup_result -eq 'PASS' -and
            -not [bool]$FinalMachineState.service_present -and
            [int64]$FinalMachineState.service_pid -eq 0 -and
            [int64]$FinalMachineState.owned_process_count -eq 0 -and
            $systemFinalSafe -and
            $profileFinalSafe
        $notRunEvidence = [ordered]@{
            schema = 'amd-i2g-real-evidence-placeholder/v1'
            qualification_only = $true
            status = 'NOT_RUN'
            reason = Get-I2gTreatmentPlaceholderReason `
                -TreatmentAllowedByScientificGate $TreatmentAllowedByScientificGate `
                -TreatmentDiscoveryStarted $TreatmentDiscoveryStarted `
                -TreatmentDiscoveryCompleted $TreatmentDiscoveryCompleted `
                -HarnessRuntimeFailure $HarnessRuntimeFailure
            treatment_allowed_by_scientific_gate = $TreatmentAllowedByScientificGate
            treatment_policy_mutation_started = $TreatmentPolicyMutationStarted
            treatment_policy_mutation_completed = $TreatmentPolicyMutationCompleted
            treatment_service_phase_started = $TreatmentServicePhaseStarted
            treatment_discovery_spawn_intent_durable = $TreatmentDiscoverySpawnIntentDurable
            treatment_discovery_started = $TreatmentDiscoveryStarted
            treatment_discovery_completed = $TreatmentDiscoveryCompleted
            harness_runtime_failure = $HarnessRuntimeFailure
            failure_class = $FailureClass
            scientific_result = $ScientificResult
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        }
        foreach ($requiredPlaceholder in @(
                'CONTROL-POLICY.json', 'CONTROL-TOKEN-PRE.json', 'CONTROL-TOKEN-POST.json',
                'CONTROL-AMD-IDENTITY.json', 'CONTROL-DISCOVERY.json', 'CONTROL-TEARDOWN.json',
                'TREATMENT-POLICY.json', 'TREATMENT-TOKEN-PRE.json',
                'TREATMENT-TOKEN-SYSTEMPROFILE.json', 'TREATMENT-TOKEN-FINAL.json',
                'TREATMENT-AMD-IDENTITY.json', 'TREATMENT-DISCOVERY.json',
                'PAIRED-CONFIG-COMPARISON.json', 'PAIRED-TOKEN-COMPARISON.json'
            )) {
            Write-I2gPlaceholderIfMissing -Path (Join-Path $RunRoot $requiredPlaceholder) -Value $notRunEvidence
        }
        Write-I2gPlaceholderIfMissing -Path (Join-Path $RunRoot 'PAIRED-RESULT.json') -Value ([ordered]@{
            schema = 'amd-i2g-paired-result/v1'
            qualification_only = $true
            control_result = $ControlResult
            treatment_result = $TreatmentResult
            paired_result = $PairedResult
            configuration_delta_pass = $PairedConfigPass
            token_delta_pass = $PairedTokenPass
            treatment_allowed_by_scientific_gate = $TreatmentAllowedByScientificGate
            treatment_policy_mutation_started = $TreatmentPolicyMutationStarted
            treatment_policy_mutation_completed = $TreatmentPolicyMutationCompleted
            treatment_service_phase_started = $TreatmentServicePhaseStarted
            treatment_service_phase_completed = if ($TreatmentServicePhaseStarted) { $TreatmentDiscoveryCompleted } else { $false }
            treatment_discovery_spawn_intent_durable = $TreatmentDiscoverySpawnIntentDurable
            treatment_discovery_started = $TreatmentDiscoveryStarted
            treatment_discovery_completed = $TreatmentDiscoveryCompleted
            failure_class = $FailureClass
            scientific_result = $ScientificResult
            cleanup_result = if ($cleanupPass) { 'PASS' } else { 'FAIL' }
            causal_interpretation_valid = $CausalInterpretationValid -and $cleanupPass
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        })
        if ($cleanupPass -and (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
            Remove-Item -LiteralPath $ConfigPath -Force
            $FinalMachineState.active_config_present = $false
        }
        if ($cleanupPass) {
            $script:I2gState.recovery_required = $false
            Set-I2gState -State 'RollbackComplete'
        }
        $overall = if (-not [string]::IsNullOrWhiteSpace($CleanupError) -or -not $cleanupPass) {
            'BLOCKED'
        } elseif ($PairedResult -in @('PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT', 'PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT')) {
            'PASS_AMD_PRIVILEGE_I2G_REAL_QUALIFICATION'
        } elseif ($HarnessRuntimeFailure) {
            'BLOCKED_HARNESS_RUNTIME_ERROR'
        } elseif ($FailureClass -eq 'SCIENTIFIC_INVALIDATION' -or $PrimaryError -like 'INVALID_NO_CAUSAL_INTERPRETATION*' -or -not $TreatmentAllowedByScientificGate) {
            'INVALID_NO_CAUSAL_INTERPRETATION'
        } else {
            'BLOCKED'
        }
        $final = [ordered]@{
            schema = 'amd-i2g-real-final-summary/v1'
            result = $overall
            task_id = 'AMD-PRIVILEGE-I2G-REAL-QUALIFICATION'
            run_id = $RunId
            human_real_run_authorization = if ($overall -eq 'PASS_AMD_PRIVILEGE_I2G_REAL_QUALIFICATION') { 'CONSUMED' } else { 'CONSUMED' }
            real_privileged_run = if ([int]$ControlRuns + [int]$TreatmentRuns -gt 0) { 'YES' } else { 'NO' }
            real_cleanup_run = if ($ServiceCreated -or $SystemProfileAddedByRun -or $ProfileSingleAddedByRun) { 'YES' } else { 'NO' }
            control_runs = $ControlRuns
            treatment_runs = $TreatmentRuns
            total_discovery_runs = [int]$ControlRuns + [int]$TreatmentRuns
            power_sampling_runs = 0
            retry_occurred = $false
            control_result = $ControlResult
            treatment_result = $TreatmentResult
            paired_result = $PairedResult
            causal_interpretation_valid = $CausalInterpretationValid -and $overall -eq 'PASS_AMD_PRIVILEGE_I2G_REAL_QUALIFICATION'
            control_token_gate = $ControlTokenGate
            treatment_token_gate = $TreatmentTokenGate
            paired_config_delta = $PairedConfigPass
            paired_token_delta = $PairedTokenPass
            treatment_allowed = $TreatmentAllowedByScientificGate
            treatment_allowed_by_scientific_gate = $TreatmentAllowedByScientificGate
            treatment_policy_mutation_started = $TreatmentPolicyMutationStarted
            treatment_policy_mutation_completed = $TreatmentPolicyMutationCompleted
            treatment_service_phase_started = $TreatmentServicePhaseStarted
            treatment_service_phase_completed = if ($TreatmentServicePhaseStarted) { $TreatmentDiscoveryCompleted } else { $false }
            treatment_discovery_spawn_intent_durable = $TreatmentDiscoverySpawnIntentDurable
            treatment_discovery_started = $TreatmentDiscoveryStarted
            treatment_discovery_completed = $TreatmentDiscoveryCompleted
            harness_runtime_failure = $HarnessRuntimeFailure
            failure_class = $FailureClass
            scientific_result = $ScientificResult
            preexisting_system_profile_right = $SystemProfilePreexisting
            preexisting_profile_single_right = $ProfileSinglePreexisting
            system_profile_added_by_run = $SystemProfileAddedByRun
            profile_single_added_by_run = $ProfileSingleAddedByRun
            system_profile_cleanup = if ($null -ne $Rollback) { $Rollback.system_profile_cleanup } else { $false }
            profile_single_cleanup = if ($null -ne $Rollback) { $Rollback.profile_single_cleanup } else { $false }
            service_cleanup = if ($null -ne $Rollback) { $Rollback.service_cleanup } else { $false }
            process_cleanup = if ($null -ne $Rollback) { $Rollback.process_cleanup } else { $false }
            final_machine_state = $FinalMachineState
            amd_cli_identity = $AmdCliIdentity
            harness_artifact_identity = $HarnessIdentity
            evidence_root = $RunRoot
            primary_error = $PrimaryError
            cleanup_error = $CleanupError
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        }
        Write-I2gAtomicJson -Path (Join-Path $RunRoot 'FINAL-SUMMARY.json') -Value $final
        $final | ConvertTo-Json -Depth 50
        if ($overall -eq 'BLOCKED' -or $overall -eq 'BLOCKED_HARNESS_RUNTIME_ERROR') { exit 1 }
        if ($overall -eq 'INVALID_NO_CAUSAL_INTERPRETATION') { exit 2 }
    }
    elseif ($null -ne $PrimaryError) {
        [ordered]@{
            result = if ($FailureClass -eq 'SCIENTIFIC_INVALIDATION' -or $PrimaryError -like 'INVALID_NO_CAUSAL_INTERPRETATION*') { 'INVALID_NO_CAUSAL_INTERPRETATION' } else { 'BLOCKED_HARNESS_RUNTIME_ERROR' }
            primary_error = $PrimaryError
            failure_class = $FailureClass
            scientific_result = $ScientificResult
            harness_runtime_failure = $HarnessRuntimeFailure
            real_privileged_run = 'NO'
            real_cleanup_run = 'NO'
            power_sampling_runs = 0
            retry_occurred = $false
        } | ConvertTo-Json -Depth 20
        if ($FailureClass -eq 'SCIENTIFIC_INVALIDATION' -or $PrimaryError -like 'INVALID_NO_CAUSAL_INTERPRETATION*') { exit 2 }
        exit 1
    }
}
