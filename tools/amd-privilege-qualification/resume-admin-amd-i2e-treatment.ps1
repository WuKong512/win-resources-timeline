#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedTreatmentOnly,
    # Internal offline-test seam. It is accepted only with the dedicated
    # test environment marker and always returns before machine mutation.
    [switch]$InternalTestOnlyPreMutationSentinel
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ResumeScriptRoot = $PSScriptRoot
. (Join-Path $ResumeScriptRoot 'i2e-runtime-library.ps1')
. (Join-Path $ResumeScriptRoot 'i2e-treatment-resume-contract.ps1')

$ServiceName = $I2eServiceName
$ServiceAccount = $I2eServiceAccount
$ServiceAccountSid = $I2eServiceAccountSid
$ServiceSidAccount = $I2eServiceSidAccount
$ScServiceAccount = 'NT AUTHORITY\LocalService'
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$QualificationRoot = Join-Path $env:ProgramData $I2eOutputSubdirectory

$ExpectedExperimentId = '3935ac9082954bcfb2b1f94c54cf95d7'
$ExpectedControlScope = '07a511e169274def93da79f269792b71'
$ExpectedTreatmentScope = 'e66bbcff49ff4aeaaf8bd2a75aa959c7'
$ExpectedServiceSid = 'S-1-5-80-2365814672-2637389132-1660472602-1496836994-3411780124'
$ExpectedArtifactSha256 = '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9'
$ExpectedServiceStartMode = '--service-profile-counter-service'
$ConfigPath = Join-Path $QualificationRoot 'I2E-CONFIG.json'
$PointerPath = Join-Path $QualificationRoot 'I2E-EXPERIMENT-CURRENT.json'

function Get-I2eResumeServiceConfiguration {
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$ServiceName'" -ErrorAction Stop |
        Select-Object -First 1
    if ($null -eq $service) { return $null }
    [pscustomobject]@{
        name = [string]$service.Name
        state = [string]$service.State
        process_id = [int64]$service.ProcessId
        start_name = [string]$service.StartName
        path_name = [string]$service.PathName
        start_mode = [string]$service.StartMode
    }
}

function Get-I2eResumeOwnedBrokerProcesses {
    $expectedPath = [IO.Path]::GetFullPath($ArtifactPath)
    @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expectedPath) } catch { $false }
    })
}

function Get-I2eResumePinnedAmdCliPath {
    $preflightPath = Join-Path (Join-Path $QualificationRoot $ExpectedExperimentId) 'AMD-CLI-PREFLIGHT.json'
    if (-not (Test-Path -LiteralPath $preflightPath -PathType Leaf)) {
        throw ('Pinned AMD CLI preflight is absent; refusing ownership inference: {0}' -f $preflightPath)
    }
    $preflight = Read-I2eJson -Path $preflightPath
    if (-not (Test-I2eResumeBoolean -Actual (Get-I2eResumeProperty -Object $preflight -Name 'preflight_pass') -Expected $true)) {
        throw ('Pinned AMD CLI preflight is not passing: {0}' -f $preflightPath)
    }
    $path = [string](Get-I2eResumeProperty -Object $preflight -Name 'path' -Default '')
    if ([string]::IsNullOrWhiteSpace($path)) {
        throw ('Pinned AMD CLI preflight has no path: {0}' -f $preflightPath)
    }
    try { return [IO.Path]::GetFullPath($path) }
    catch { throw ('Pinned AMD CLI preflight path is invalid: {0}' -f $path) }
}

function Get-I2eResumeAmdProcesses {
    param([Parameter(Mandatory = $true)][string]$ExpectedAmdCliPath)
    $expectedPath = [IO.Path]::GetFullPath($ExpectedAmdCliPath)
    @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expectedPath) } catch { $false }
    })
}

function Read-I2eResumeEvidence {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw ('Required I2E treatment-resume evidence is absent: {0}' -f $Path)
    }
    Read-I2eJson -Path $Path
}

function Assert-I2eTreatmentResumeServiceGate {
    $service = Get-I2eResumeServiceConfiguration
    if ($null -eq $service) { throw ('Treatment resume requires the existing service: {0}' -f $ServiceName) }
    if ($service.state -cne 'Stopped' -or $service.process_id -ne 0) {
        throw ('Treatment resume requires service Stopped/PID0; state={0}, process_id={1}' -f $service.state, $service.process_id)
    }
    if ($service.start_name -ine 'NT AUTHORITY\LocalService') {
        throw ('Treatment resume requires NT AUTHORITY\LocalService; actual={0}' -f $service.start_name)
    }
    if ($service.path_name -notmatch ('(?i)' + [regex]::Escape($ArtifactPath)) -or
        $service.path_name -notmatch ('(?i)' + [regex]::Escape($ExpectedServiceStartMode))) {
        throw ('Treatment resume service image/mode mismatch: {0}' -f $service.path_name)
    }
    Assert-I2eServiceSidType -ServiceName $ServiceName
    $resolvedSid = Resolve-I2eServiceSid -ServiceSidAccount $ServiceSidAccount
    if ($resolvedSid -cne $ExpectedServiceSid) {
        throw ('Treatment resume Service SID mismatch; expected={0}, actual={1}' -f $ExpectedServiceSid, $resolvedSid)
    }
    [pscustomobject]@{
        service_name = $service.name
        state = $service.state
        process_id = $service.process_id
        start_name = $service.start_name
        service_sid = $resolvedSid
        service_sid_type = 'UNRESTRICTED'
        path_name = $service.path_name
        start_mode = $service.start_mode
    }
}

function Assert-I2eTreatmentResumeArtifact {
    if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
        throw ('Missing frozen treatment artifact: {0}' -f $ArtifactPath)
    }
    $hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($hash -cne $ExpectedArtifactSha256) {
        throw ('Treatment artifact hash mismatch; expected={0}, actual={1}' -f $ExpectedArtifactSha256, $hash)
    }
    if ((Get-I2ePeArchitecture -Path $ArtifactPath) -cne 'x64') {
        throw 'Treatment artifact must be x64.'
    }
    $hash
}

function Assert-I2eTreatmentResumePointer {
    if (-not (Test-Path -LiteralPath $PointerPath -PathType Leaf)) {
        throw ('Treatment resume requires the existing CURRENT pointer: {0}' -f $PointerPath)
    }
    $pointer = Read-I2eJson -Path $PointerPath
    foreach ($check in @(
        @{ Name = 'experiment_id'; Actual = Get-I2eResumeProperty $pointer 'experiment_id'; Expected = $ExpectedExperimentId },
        @{ Name = 'control_scope'; Actual = Get-I2eResumeProperty $pointer 'control_scope'; Expected = $ExpectedControlScope },
        @{ Name = 'treatment_scope'; Actual = Get-I2eResumeProperty $pointer 'treatment_scope'; Expected = $ExpectedTreatmentScope },
        @{ Name = 'artifact_sha256'; Actual = Get-I2eResumeProperty $pointer 'artifact_sha256'; Expected = $ExpectedArtifactSha256 },
        @{ Name = 'service_name'; Actual = Get-I2eResumeProperty $pointer 'service_name'; Expected = $ServiceName },
        @{ Name = 'service_sid'; Actual = Get-I2eResumeProperty $pointer 'service_sid'; Expected = $ExpectedServiceSid }
    )) {
        if (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            throw ('Treatment resume pointer mismatch: {0}' -f $check.Name)
        }
    }
    if ((Test-I2eResumeBoolean (Get-I2eResumeProperty $pointer 'treatment_executed') $true) -or
        (Test-I2eResumeBoolean (Get-I2eResumeProperty $pointer 'right_added_by_experiment') $true)) {
        throw 'Treatment resume refuses an already-mutated or already-executed treatment.'
    }
    $pointer
}

function Get-I2eControlRecoveryEvidence {
    param([Parameter(Mandatory = $true)]$Pointer)
    $experimentRoot = Join-Path $QualificationRoot $ExpectedExperimentId
    $controlRoot = Join-Path $QualificationRoot $ExpectedControlScope
    $paths = Assert-I2eControlRecoveryEvidenceFiles -ControlRoot $controlRoot
    $tokenGate = Read-I2eResumeEvidence -Path $paths.token_gate
    $context = Read-I2eResumeEvidence -Path $paths.context
    $summary = Read-I2eResumeEvidence -Path $paths.summary
    $discovery = Read-I2eResumeEvidence -Path $paths.discovery_result
    $launch = Read-I2eResumeEvidence -Path $paths.discovery_launch
    $harness = if (Test-Path -LiteralPath $paths.harness_error -PathType Leaf) { Read-I2eJson -Path $paths.harness_error } else { $null }
    $recovery = Assert-I2eControlRecoveryEvidence `
        -Pointer $Pointer `
        -ControlTokenGate $tokenGate `
        -ControlContext $context `
        -ControlSummary $summary `
        -ControlDiscoveryResult $discovery `
        -ControlLaunch $launch `
        -ControlHarnessError $harness `
        -ExpectedExperimentId $ExpectedExperimentId `
        -ExpectedControlScope $ExpectedControlScope `
        -ExpectedTreatmentScope $ExpectedTreatmentScope `
        -ExpectedArtifactSha256 $ExpectedArtifactSha256 `
        -ExpectedServiceSid $ExpectedServiceSid `
        -ExpectedServiceName $ServiceName
    [pscustomobject]@{
        root = $experimentRoot
        control_root = $controlRoot
        token_gate = $tokenGate
        context = $context
        summary = $summary
        discovery = $discovery
        launch = $launch
        recovery = $recovery
    }
}

function Write-I2eControlRecovery {
    param(
        [Parameter(Mandatory = $true)]$Pointer,
        [Parameter(Mandatory = $true)]$ControlEvidence
    )
    $experimentRoot = Join-Path $QualificationRoot $ExpectedExperimentId
    $recoveryPath = Join-Path $experimentRoot 'CONTROL-RECOVERY.json'
    $authoritativeSources = @(
        (Join-Path $ControlEvidence.control_root 'SERVICE-PROFILE-TOKEN-GATE.json'),
        (Join-Path $ControlEvidence.control_root 'SERVICE-PROFILE-SERVICE-CONTEXT.json'),
        (Join-Path $ControlEvidence.control_root 'SERVICE-PROFILE-COUNTER-SUMMARY.json'),
        (Join-Path $ControlEvidence.control_root 'AMD-COUNTER-DISCOVERY-RESULT.json'),
        (Join-Path $ControlEvidence.control_root 'AMD-COUNTER-DISCOVERY-LAUNCH.json')
    )
    if (Test-Path -LiteralPath $recoveryPath -PathType Leaf) {
        $existing = Read-I2eJson -Path $recoveryPath
        foreach ($check in @(
                @{ Name = 'schema'; Actual = Get-I2eResumeProperty $existing 'schema'; Expected = 'amd-service-profile-control-recovery/v1' },
                @{ Name = 'experiment_id'; Actual = Get-I2eResumeProperty $existing 'experiment_id'; Expected = $ExpectedExperimentId },
                @{ Name = 'control_scope'; Actual = Get-I2eResumeProperty $existing 'control_scope'; Expected = $ExpectedControlScope },
                @{ Name = 'control_result'; Actual = Get-I2eResumeProperty $existing 'control_result'; Expected = 'POWER_UNAVAILABLE' }
            )) {
            if (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
                throw ('Existing CONTROL-RECOVERY.json mismatch: {0}' -f $check.Name)
            }
        }
        if (-not (Test-I2eResumeBoolean (Get-I2eResumeProperty $existing 'control_real_executed') $true) -or
            -not (Test-I2eResumeBoolean (Get-I2eResumeProperty $existing 'paired_gate_consumed') $true) -or
            (@(Get-I2eResumeProperty $existing 'authoritative_sources' -Default @()) -join "`n") -cne ($authoritativeSources -join "`n")) {
            throw 'Existing CONTROL-RECOVERY.json does not validate against the authoritative CONTROL evidence.'
        }
    }
    else {
        Write-I2eJson -Path $recoveryPath -Value ([ordered]@{
            schema = 'amd-service-profile-control-recovery/v1'
            qualification_only = $true
            experiment_id = $ExpectedExperimentId
            control_scope = $ExpectedControlScope
            original_pointer_state = [string](Get-I2eResumeProperty $Pointer 'state' -Default 'UNKNOWN')
            original_control_execution_state = [string](Get-I2eResumeProperty $Pointer 'control_execution_state' -Default 'UNKNOWN')
            recovery_reason = 'POWERSHELL_AUTOMATIC_VARIABLE_PID_COLLISION_AFTER_REAL_CONTROL'
            authoritative_sources = $authoritativeSources
            control_real_executed = $true
            control_result = 'POWER_UNAVAILABLE'
            paired_gate_consumed = $true
            recovered_at_utc = [DateTime]::UtcNow.ToString('o')
        })
    }
    Set-I2eObjectProperty -Object $Pointer -Name 'control_execution_state' -Value 'COMPLETED_RECOVERED' | Out-Null
    Set-I2eObjectProperty -Object $Pointer -Name 'control_executed' -Value $true | Out-Null
    Set-I2eObjectProperty -Object $Pointer -Name 'control_result' -Value 'POWER_UNAVAILABLE' | Out-Null
    Set-I2eObjectProperty -Object $Pointer -Name 'paired_gate_consumed' -Value $true | Out-Null
    Set-I2eObjectProperty -Object $Pointer -Name 'state' -Value 'CONTROL_EXECUTED_RECOVERED' | Out-Null
    Write-I2eJson -Path $PointerPath -Value $Pointer
}

function Assert-I2eTreatmentResumeSecurityGate {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)
    $direct = Get-I2eDirectAccountRightsSnapshot -Label 'treatment-before-add' -Sid $ServiceSid
    $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($direct.status -ne 'READ' -or $assigned.status -ne 'READ') {
        throw 'Treatment resume could not read both exact-right directions.'
    }
    if (@($direct.direct_rights) -contains $I2eRequiredRight -or
        @($assigned.assigned_principals) -contains $ServiceSid) {
        throw 'Treatment resume found a pre-existing SeSystemProfilePrivilege assignment; refusing mutation.'
    }
    [pscustomobject]@{ direct = $direct; assigned = $assigned }
}

function Assert-I2eTreatmentResumeNoOwnedProcesses {
    $expectedAmdCliPath = Get-I2eResumePinnedAmdCliPath
    if (@(Get-I2eResumeOwnedBrokerProcesses).Count -ne 0) {
        throw 'Treatment resume found an owned qualification broker process.'
    }
    if (@(Get-I2eResumeAmdProcesses -ExpectedAmdCliPath $expectedAmdCliPath).Count -ne 0) {
        throw 'Treatment resume found an owned AMD CLI process.'
    }
}

function Assert-I2eTreatmentEvidenceAbsent {
    $treatmentRoot = Join-Path $QualificationRoot $ExpectedTreatmentScope
    foreach ($name in @(
        'SERVICE-PROFILE-SERVICE-CONTEXT.json',
        'SERVICE-PROFILE-TOKEN-GATE.json',
        'SERVICE-PROFILE-COUNTER-SUMMARY.json',
        'AMD-COUNTER-DISCOVERY-LAUNCH.json',
        'AMD-COUNTER-DISCOVERY-RESULT.json',
        'SERVICE-PROFILE-SERVICE-HARNESS-ERROR.json'
    )) {
        if (Test-Path -LiteralPath (Join-Path $treatmentRoot $name) -PathType Leaf) {
            throw ('Treatment evidence already exists; refusing a second treatment: {0}' -f $name)
        }
    }
    $discoveryRoot = Join-Path $treatmentRoot 'counter-discovery'
    foreach ($name in @('AMD-COUNTER-DISCOVERY-LAUNCH.json', 'AMD-COUNTER-DISCOVERY-RESULT.json')) {
        if (Test-Path -LiteralPath (Join-Path $discoveryRoot $name) -PathType Leaf) {
            throw ('Treatment discovery evidence already exists; refusing a second treatment: {0}' -f $name)
        }
    }
}

function Write-I2eTreatmentMutationApplied {
    param(
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)]$Before
    )
    $experimentRoot = Join-Path $QualificationRoot $ExpectedExperimentId
    $afterDirect = Get-I2eDirectAccountRightsSnapshot -Label 'treatment-after-add' -Sid $ServiceSid
    $afterAssigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($afterDirect.status -ne 'READ' -or $afterAssigned.status -ne 'READ' -or
        @($afterDirect.direct_rights) -notcontains $I2eRequiredRight -or
        @($afterAssigned.assigned_principals) -notcontains $ServiceSid) {
        throw 'Treatment exact-right assignment was not verified in both directions.'
    }
    Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-APPLIED.json') -Value ([ordered]@{
        schema = 'amd-service-profile-security-mutation-applied/v1'
        qualification_only = $true
        experiment_id = $ExpectedExperimentId
        service_name = $ServiceName
        service_sid = $ServiceSid
        right = $I2eRequiredRight
        right_was_present_before = $false
        right_added_by_experiment = $true
        applied_at_utc = [DateTime]::UtcNow.ToString('o')
        baseline = $Before
        direct_verification = $afterDirect
        assignment_verification = $afterAssigned
    })
    [pscustomobject]@{ direct = $afterDirect; assigned = $afterAssigned }
}

function Invoke-I2eTreatmentRollback {
    param(
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)][string]$ExperimentRoot,
        [Parameter(Mandatory = $true)][bool]$RightAddedByExperiment
    )

    $stopResult = $null
    $stopError = $null
    try {
        $stopResult = Stop-I2eService -ServiceName $ServiceName
    } catch {
        $stopError = $_.Exception.Message
    }

    $service = $null
    $serviceState = 'UNKNOWN'
    $serviceProcessId = -1L
    try {
        $service = Get-I2eResumeServiceConfiguration
        if ($null -eq $service) {
            $serviceState = 'ABSENT'
            $serviceProcessId = 0L
        } else {
            $serviceState = [string]$service.state
            $serviceProcessId = [int64]$service.process_id
        }
    } catch {
        $stopError = if ($null -eq $stopError) { $_.Exception.Message } else { '{0}; service-state-read: {1}' -f $stopError, $_.Exception.Message }
    }
    $ownedBrokerCount = @(Get-I2eResumeOwnedBrokerProcesses).Count
    $amdCliCount = @(Get-I2eResumeAmdProcesses -ExpectedAmdCliPath (Get-I2eResumePinnedAmdCliPath)).Count

    $direct = $null
    $assigned = $null
    $policyRemoveAttempted = $false
    $policyRollbackVerified = -not $RightAddedByExperiment
    $policyError = $null
    if ($RightAddedByExperiment) {
        $policyRemoveAttempted = $true
        try {
            Remove-I2eExactServiceProfileRight -ServiceSid $ServiceSid
            $direct = Get-I2eDirectAccountRightsSnapshot -Label 'treatment-after-rollback' -Sid $ServiceSid
            $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
            $policyRollbackVerified = $direct.status -eq 'READ' -and $assigned.status -eq 'READ' -and
                @($direct.direct_rights) -notcontains $I2eRequiredRight -and
                @($assigned.assigned_principals) -notcontains $ServiceSid
            if (-not $policyRollbackVerified) {
                $policyError = 'Treatment exact-right rollback was not verified in both LSA readback directions.'
            }
        } catch {
            $policyError = $_.Exception.Message
            $policyRollbackVerified = $false
        }
    }

    $verification = Get-I2eRollbackVerification `
        -PolicyRollbackVerified $policyRollbackVerified `
        -ServicePresent ($null -ne $service) `
        -ServiceState $serviceState `
        -ServiceProcessId $serviceProcessId `
        -OwnedBrokerProcessCount $ownedBrokerCount `
        -AmdCliProcessCount $amdCliCount
    $serviceRemoved = $false
    $serviceRemovalError = $null
    $rollbackEvidencePath = Join-Path $ExperimentRoot 'SECURITY-MUTATION-ROLLBACK.json'
    $writeRollbackEvidence = {
        param([bool]$CurrentServiceRemoved)
        $evidence = [ordered]@{
        schema = 'amd-service-profile-security-mutation-rollback/v1'
        qualification_only = $true
        experiment_id = $ExpectedExperimentId
        service_name = $ServiceName
        service_sid = $ServiceSid
        right = $I2eRequiredRight
        right_added_by_experiment = $RightAddedByExperiment
        all_rights = $false
        service_stop_attempted = $true
        service_stop_verified = $verification.service_stop_verified
        service_state_after_stop = $serviceState
        service_pid_after_stop = $serviceProcessId
        owned_broker_process_count_after_stop = $ownedBrokerCount
        amd_cli_process_count_after_stop = $amdCliCount
        policy_remove_attempted = $policyRemoveAttempted
        policy_right_removed = $policyRollbackVerified
        policy_rollback_verified = $policyRollbackVerified
        effective_token_teardown_verified = $verification.effective_token_teardown_verified
        full_rollback_verified = $verification.full_rollback_verified
        service_registration_removed = $CurrentServiceRemoved
        rollback_at_utc = [DateTime]::UtcNow.ToString('o')
        rollback_verified = $verification.full_rollback_verified
        direct_verification = $direct
        assignment_verification = $assigned
        }
        if ($null -ne $stopError) { $evidence.stop_error = $stopError }
        if ($null -ne $policyError) { $evidence.policy_error = $policyError }
        if ($null -ne $serviceRemovalError) { $evidence.service_removal_error = $serviceRemovalError }
        Write-I2eJson -Path $rollbackEvidencePath -Value $evidence
    }
    & $writeRollbackEvidence $false

    if ($verification.full_rollback_verified -and $null -ne $service) {
        try {
            Remove-I2eService -ServiceName $ServiceName
            $serviceRemoved = $true
        } catch {
            $serviceRemovalError = $_.Exception.Message
        }
        & $writeRollbackEvidence $serviceRemoved
    }

    [pscustomobject]@{
        stop_result = $stopResult
        service_stop_attempted = $true
        service_stop_verified = [bool]$verification.service_stop_verified
        service_state_after_stop = $serviceState
        service_pid_after_stop = $serviceProcessId
        owned_broker_process_count_after_stop = $ownedBrokerCount
        amd_cli_process_count_after_stop = $amdCliCount
        policy_remove_attempted = $policyRemoveAttempted
        policy_rollback_verified = [bool]$policyRollbackVerified
        effective_token_teardown_verified = [bool]$verification.effective_token_teardown_verified
        full_rollback_verified = [bool]$verification.full_rollback_verified
        service_registration_removed = $serviceRemoved -or $null -eq $service
        direct_verification = $direct
        assignment_verification = $assigned
        stop_error = $stopError
        policy_error = $policyError
        service_removal_error = $serviceRemovalError
    }
}

function Close-I2eTreatmentPointer {
    param(
        [Parameter(Mandatory = $true)]$Pointer,
        [Parameter(Mandatory = $true)][string]$ExperimentRoot,
        [Parameter(Mandatory = $true)][string]$TreatmentResult
    )
    $attempt = '{0}-{1}' -f [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'), [Guid]::NewGuid().ToString('N')
    $finalPath = Join-Path $ExperimentRoot ('I2E-EXPERIMENT-FINAL-{0}.json' -f $attempt)
    $final = [ordered]@{}
    foreach ($property in $Pointer.PSObject.Properties) { $final[$property.Name] = $property.Value }
    $final.schema = 'amd-service-profile-experiment-final/v1'
    $final.state = 'CLOSED'
    $final.control_execution_state = 'COMPLETED_RECOVERED'
    $final.control_executed = $true
    $final.control_result = 'POWER_UNAVAILABLE'
    $final.treatment_execution_state = 'COMPLETED'
    $final.treatment_executed = $true
    $final.treatment_result = $TreatmentResult
    $final.right_added_by_experiment = $true
    $final.policy_rollback_verified = $true
    $final.effective_token_teardown_verified = $true
    $final.full_rollback_verified = $true
    $final.rollback_verified = $true
    $final.paired_gate_consumed = $true
    $final.experiment_closed = $true
    $final.cleanup_attempt = $attempt
    $final.current_pointer_removed = $false
    $final.closed_at_utc = [DateTime]::UtcNow.ToString('o')
    Write-I2eJson -Path $finalPath -Value $final
    Remove-Item -LiteralPath $PointerPath -Force -ErrorAction Stop
    if (Test-Path -LiteralPath $PointerPath -PathType Leaf) {
        throw 'I2E CURRENT pointer remained after treatment finalization.'
    }
    $final.current_pointer_removed = $true
    $final.finalization_verified_at_utc = [DateTime]::UtcNow.ToString('o')
    Write-I2eJson -Path $finalPath -Value $final
    $finalPath
}

function Get-I2eTreatmentResumePlan {
    [ordered]@{
        schema = 'amd-service-profile-treatment-resume-plan/v1'
        qualification_only = $true
        experiment_id = $ExpectedExperimentId
        service_name = $ServiceName
        service_account = $ServiceAccount
        service_account_sid = $ServiceAccountSid
        service_sid = $ExpectedServiceSid
        control_scope = $ExpectedControlScope
        treatment_scope = $ExpectedTreatmentScope
        fixed_cli_arguments = $I2eFixedArguments
        sampling = $false
        control_rerun_allowed = $false
        treatment_only = $true
        exact_right = $I2eRequiredRight
        artifact_sha256 = $ExpectedArtifactSha256
        no_production_account_switch = $true
    }
}

if ($InternalTestOnlyPreMutationSentinel -and $env:I2E_RESUME_OFFLINE_TEST_SENTINEL -cne 'true') {
    throw 'The I2E treatment-resume pre-mutation sentinel is restricted to the offline test environment.'
}
if ($InternalTestOnlyPreMutationSentinel -and -not $ExecuteAuthorizedTreatmentOnly) {
    throw 'The I2E treatment-resume pre-mutation sentinel requires -ExecuteAuthorizedTreatmentOnly.'
}
if (-not $ExecuteAuthorizedTreatmentOnly) {
    Get-I2eTreatmentResumePlan | ConvertTo-Json -Depth 20
    Write-Host 'I2E_TREATMENT_RESUME_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, or AMD runtime was performed.'
    return
}
if ($InternalTestOnlyPreMutationSentinel) {
    Write-Host 'I2E_TREATMENT_RESUME_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    Write-Host 'No service, LSA mutation, token adjustment, or AMD runtime was performed.'
    return
}

$null = Assert-I2eAdministrator
$artifactHash = Assert-I2eTreatmentResumeArtifact
$pointer = Assert-I2eTreatmentResumePointer
$serviceGate = Assert-I2eTreatmentResumeServiceGate
Assert-I2eTreatmentResumeNoOwnedProcesses
Assert-I2eTreatmentEvidenceAbsent
$controlEvidence = Get-I2eControlRecoveryEvidence -Pointer $pointer
$experimentRoot = Join-Path $QualificationRoot $ExpectedExperimentId
$controlAmdPreflightPath = Join-Path $experimentRoot 'AMD-CLI-PREFLIGHT.json'
$controlAmdPreflight = Read-I2eResumeEvidence -Path $controlAmdPreflightPath
$currentAmdPreflight = [pscustomobject]@{ preflight_pass = $false }
$amdPreflightError = $null
try {
    $currentAmdPreflight = Get-I2eAmdCliPreflight
} catch {
    $amdPreflightError = $_.Exception.Message
}
$amdPreflightComparison = Compare-I2eAmdCliPreflight -Control $controlAmdPreflight -Current $currentAmdPreflight
$amdPreflightEvidence = [ordered]@{
    schema = 'amd-service-profile-treatment-amd-cli-preflight/v1'
    qualification_only = $true
    experiment_id = $ExpectedExperimentId
    control_preflight_path = $controlAmdPreflightPath
    control_identity = $controlAmdPreflight
    current_identity = $currentAmdPreflight
    comparison = $amdPreflightComparison.comparison
    differing_fields = @($amdPreflightComparison.differing_fields)
    pass = ($null -eq $amdPreflightError -and [bool]$amdPreflightComparison.pass)
    recorded_at_utc = [DateTime]::UtcNow.ToString('o')
}
if ($null -ne $amdPreflightError) {
    $amdPreflightEvidence.error = $amdPreflightError
}
Write-I2eJson -Path (Join-Path $experimentRoot 'TREATMENT-AMD-CLI-PREFLIGHT.json') -Value $amdPreflightEvidence
if (-not $amdPreflightEvidence.pass) {
    $details = if ($null -ne $amdPreflightError) { $amdPreflightError } else { ($amdPreflightComparison.differing_fields -join ', ') }
    throw ('AMD CLI identity changed between CONTROL and TREATMENT; refusing LSA mutation. Differences: {0}' -f $details)
}
$before = Assert-I2eTreatmentResumeSecurityGate -ServiceSid $ExpectedServiceSid
Write-I2eControlRecovery -Pointer $pointer -ControlEvidence $controlEvidence
Set-I2eObjectProperty -Object $pointer -Name 'right_mutation_state' -Value 'STARTING' | Out-Null
Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'RIGHT_MUTATION_PENDING' | Out-Null
Set-I2eObjectProperty -Object $pointer -Name 'policy_rollback_verified' -Value $false | Out-Null
Set-I2eObjectProperty -Object $pointer -Name 'effective_token_teardown_verified' -Value $false | Out-Null
Set-I2eObjectProperty -Object $pointer -Name 'full_rollback_verified' -Value $false | Out-Null
Set-I2eObjectProperty -Object $pointer -Name 'service_registration_removed' -Value $false | Out-Null
Set-I2eObjectProperty -Object $pointer -Name 'rollback_verified' -Value $false | Out-Null
Write-I2eJson -Path $PointerPath -Value $pointer
$rightAdded = $false
$treatment = $null
$rollbackVerified = $false
$primaryError = $null
$cleanupError = $null
try {
    Add-I2eExactServiceProfileRight -ServiceSid $ExpectedServiceSid
    $rightAdded = $true
    Set-I2eObjectProperty -Object $pointer -Name 'right_added_by_experiment' -Value $true | Out-Null
    Set-I2eObjectProperty -Object $pointer -Name 'right_mutation_state' -Value 'COMPLETED' | Out-Null
    Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'RIGHT_MUTATED' | Out-Null
    Write-I2eJson -Path $PointerPath -Value $pointer
    $null = Write-I2eTreatmentMutationApplied -ServiceSid $ExpectedServiceSid -Before $before

    Set-I2eObjectProperty -Object $pointer -Name 'treatment_execution_state' -Value 'STARTING' | Out-Null
    Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'TREATMENT_EXECUTING' | Out-Null
    Write-I2eJson -Path $PointerPath -Value $pointer
    $treatment = Invoke-I2ePhase -Phase TREATMENT -Scope $ExpectedTreatmentScope -OutputRoot (Join-Path $QualificationRoot $ExpectedTreatmentScope) -ServiceSid $ExpectedServiceSid
    Assert-I2eTreatmentTokenGate -TokenGate $treatment.token_gate -Context $treatment.context -ExpectedServiceSid $ExpectedServiceSid
    Set-I2eObjectProperty -Object $pointer -Name 'treatment_execution_state' -Value 'COMPLETED' | Out-Null
    Set-I2eObjectProperty -Object $pointer -Name 'treatment_executed' -Value $true | Out-Null
    Set-I2eObjectProperty -Object $pointer -Name 'treatment_result' -Value ([string]$treatment.summary.availability) | Out-Null
    Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'TREATMENT_EXECUTED' | Out-Null
    Write-I2eJson -Path $PointerPath -Value $pointer
    Write-I2eJson -Path (Join-Path $experimentRoot 'TREATMENT-RESULT.json') -Value $treatment.summary
    $delta = Compare-I2eTokenDelta -ControlContext $controlEvidence.context -TreatmentContext $treatment.context
    Write-I2eJson -Path (Join-Path $experimentRoot 'TOKEN-DELTA.json') -Value $delta
    if (-not $delta.pass) { throw 'Treatment token delta was not exactly SeSystemProfilePrivilege.' }
}
catch {
    $primaryError = $_.Exception
}
finally {
    try {
        if ($rightAdded) {
            $rollbackResult = Invoke-I2eTreatmentRollback `
                -ServiceSid $ExpectedServiceSid `
                -ExperimentRoot $experimentRoot `
                -RightAddedByExperiment $true
            $rollbackVerified = [bool]$rollbackResult.full_rollback_verified
            Set-I2eObjectProperty -Object $pointer -Name 'policy_rollback_verified' -Value $rollbackResult.policy_rollback_verified | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'effective_token_teardown_verified' -Value $rollbackResult.effective_token_teardown_verified | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'full_rollback_verified' -Value $rollbackResult.full_rollback_verified | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'service_stop_attempted' -Value $rollbackResult.service_stop_attempted | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'service_stop_verified' -Value $rollbackResult.service_stop_verified | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'service_state_after_stop' -Value $rollbackResult.service_state_after_stop | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'service_pid_after_stop' -Value $rollbackResult.service_pid_after_stop | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'owned_broker_process_count_after_stop' -Value $rollbackResult.owned_broker_process_count_after_stop | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'amd_cli_process_count_after_stop' -Value $rollbackResult.amd_cli_process_count_after_stop | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'service_registration_removed' -Value $rollbackResult.service_registration_removed | Out-Null
            Set-I2eObjectProperty -Object $pointer -Name 'rollback_verified' -Value $rollbackVerified | Out-Null
            if ($rollbackResult.policy_rollback_verified) { Set-I2eObjectProperty -Object $pointer -Name 'right_mutation_state' -Value 'ROLLED_BACK' | Out-Null }
            if ($rollbackResult.full_rollback_verified) { Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'FULL_ROLLBACK_COMPLETE' | Out-Null }
            elseif ($rollbackResult.policy_rollback_verified) { Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'POLICY_ROLLBACK_COMPLETE' | Out-Null }
            else { Set-I2eObjectProperty -Object $pointer -Name 'state' -Value 'SERVICE_STOP_OR_POLICY_ROLLBACK_PENDING' | Out-Null }
            if (-not $rollbackResult.full_rollback_verified -or -not $rollbackResult.service_registration_removed) {
                $cleanupError = [Exception]::new('I2E treatment cleanup did not prove full rollback and service removal; CURRENT pointer retained.')
            }
        }
        Write-I2eJson -Path $PointerPath -Value $pointer
    }
    catch { $cleanupError = $_.Exception }
}

if ($null -ne $cleanupError) { throw ('I2E treatment cleanup failed closed: {0}' -f $cleanupError.Message) }
if ($null -ne $primaryError) { throw $primaryError }

if ($null -eq $treatment -or -not $pointer.treatment_executed -or -not $rollbackVerified) {
    throw 'I2E treatment did not reach a final authoritative state.'
}
$finalPath = Close-I2eTreatmentPointer -Pointer $pointer -ExperimentRoot $experimentRoot -TreatmentResult ([string]$treatment.summary.availability)
Write-Host ('I2E treatment-only resume completed; final evidence: {0}' -f $finalPath)
