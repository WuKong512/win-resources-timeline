#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedExperiment,
    [switch]$LibraryOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'i2e-runtime-library.ps1')

$ServiceName = $I2eServiceName
$ServiceAccount = $I2eServiceAccount
$ScServiceAccount = 'NT AUTHORITY\LocalService'
if ($ScServiceAccount -cne 'NT AUTHORITY\LocalService') {
    throw 'I2E SCM service account must use the Windows predefined LocalService identity form.'
}
$ServiceSidAccount = $I2eServiceSidAccount
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9'
$QualificationRoot = Join-Path $env:ProgramData $I2eOutputSubdirectory
$ConfigPath = Join-Path $QualificationRoot 'I2E-CONFIG.json'
$PointerPath = Join-Path $QualificationRoot 'I2E-EXPERIMENT-CURRENT.json'

function Invoke-I2ePhase {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('CONTROL', 'TREATMENT')][string]$Phase,
        [Parameter(Mandatory = $true)][string]$Scope,
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )
    $config = Get-I2ePhaseConfig -Phase $Phase -Scope $Scope -OutputRoot $OutputRoot -ServiceSid $ServiceSid
    Write-I2eJson -Path $ConfigPath -Value $config
    Write-I2eJson -Path (Join-Path $OutputRoot 'I2E-PHASE-CONFIG.json') -Value $config
    Invoke-I2eSc -Arguments @('start', $ServiceName) | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    do {
        $summaryPath = Join-Path $OutputRoot 'SERVICE-PROFILE-COUNTER-SUMMARY.json'
        $errorPath = Join-Path $OutputRoot 'SERVICE-PROFILE-SERVICE-HARNESS-ERROR.json'
        if ((Test-Path -LiteralPath $summaryPath -PathType Leaf) -or (Test-Path -LiteralPath $errorPath -PathType Leaf)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $errorPath = Join-Path $OutputRoot 'SERVICE-PROFILE-SERVICE-HARNESS-ERROR.json'
    if (Test-Path -LiteralPath $errorPath -PathType Leaf) {
        throw ('{0} service harness failed: {1}' -f $Phase, (Get-Content -LiteralPath $errorPath -Raw))
    }
    $summaryPath = Join-Path $OutputRoot 'SERVICE-PROFILE-COUNTER-SUMMARY.json'
    if (-not (Test-Path -LiteralPath $summaryPath -PathType Leaf)) {
        throw ('{0} did not produce a bounded counter-discovery result.' -f $Phase)
    }
    $stop = Stop-I2eService -ServiceName $ServiceName
    [pscustomobject]@{
        phase = $Phase
        output_root = $OutputRoot
        summary = Read-I2eJson -Path $summaryPath
        context = Read-I2eJson -Path (Join-Path $OutputRoot 'SERVICE-PROFILE-SERVICE-CONTEXT.json')
        token_gate = Read-I2eJson -Path (Join-Path $OutputRoot 'SERVICE-PROFILE-TOKEN-GATE.json')
        stop = $stop
    }
}

if ($LibraryOnly) { return }

$null = Assert-I2eAdministrator
if (-not $ExecuteAuthorizedExperiment) {
    Get-I2eExperimentPlan -ArtifactSha256 $ExpectedArtifactSha256 | ConvertTo-Json -Depth 20
    Write-Host 'I2E_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, or AMD runtime was performed.'
    return
}

if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) { throw ('Missing artifact: {0}' -f $ArtifactPath) }
$artifactHash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
if ($artifactHash -ne $ExpectedArtifactSha256.ToUpperInvariant()) {
    throw ('Artifact hash mismatch; expected={0}, actual={1}' -f $ExpectedArtifactSha256, $artifactHash)
}
if ((Get-I2ePeArchitecture -Path $ArtifactPath) -ne 'x64') { throw 'I2E artifact must be x64.' }
if ((Get-I2eServiceSnapshot -ServiceName $ServiceName).present) { throw ('Service already exists: {0}' -f $ServiceName) }
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) { throw ('Stale config exists: {0}' -f $ConfigPath) }
if (Test-Path -LiteralPath $PointerPath -PathType Leaf) { throw ('Stale experiment pointer exists: {0}' -f $PointerPath) }
$amdCliPreflight = Get-I2eAmdCliPreflight
if (-not $amdCliPreflight.preflight_pass) { throw 'AMD CLI preflight failed.' }

$experimentId = [Guid]::NewGuid().ToString('N')
$controlScope = [Guid]::NewGuid().ToString('N')
$treatmentScope = [Guid]::NewGuid().ToString('N')
$experimentRoot = Join-Path $QualificationRoot $experimentId
$controlRoot = Join-Path $QualificationRoot $controlScope
$treatmentRoot = Join-Path $QualificationRoot $treatmentScope
$serviceCreated = $false
$rightAdded = $false
$rollbackVerified = $false
$serviceSid = $null
$primaryError = $null
$cleanupError = $null
$pointer = [ordered]@{
    schema = 'amd-service-profile-experiment-current/v1'
    qualification_only = $true
    experiment_id = $experimentId
    service_name = $ServiceName
    service_account = $ServiceAccount
    service_account_sid = $I2eServiceAccountSid
    service_sid = $null
    service_sid_account = $ServiceSidAccount
    right = $I2eRequiredRight
    artifact_path = $ArtifactPath
    artifact_sha256 = $artifactHash
    control_scope = $controlScope
    control_output_root = $controlRoot
    treatment_scope = $treatmentScope
    treatment_output_root = $treatmentRoot
    state = 'PRE_SERVICE_CREATE'
    service_create_succeeded = $false
    service_sid_resolved = $false
    control_execution_state = 'NOT_STARTED'
    control_executed = $false
    control_result = $null
    paired_gate_consumed = $false
    right_mutation_state = 'NOT_STARTED'
    right_added_by_experiment = $false
    treatment_execution_state = 'NOT_STARTED'
    treatment_executed = $false
    treatment_result = $null
    rollback_verified = $false
    experiment_closed = $false
}

function Save-I2eExperimentPointer {
    Write-I2eJson -Path $PointerPath -Value $pointer
}

try {
    # AUTHORIZED_ORDER: SERVICE_CREATE < SIDTYPE_UNRESTRICTED < QSIDTYPE_VERIFY < SERVICE_SID_RESOLUTION < ACL_CONFIG < CONTROL < RIGHT_MUTATION < TREATMENT < ROLLBACK < SERVICE_DELETE
    New-Item -ItemType Directory -Force -Path $QualificationRoot, $experimentRoot, $controlRoot, $treatmentRoot | Out-Null
    Write-I2eJson -Path $PointerPath -Value $pointer
    $binPath = '"{0}" --service-profile-counter-service' -f $ArtifactPath
    $createArgs = New-QualificationServiceCreateArguments -ServiceName $ServiceName -BinPath $binPath -ServiceAccount $ScServiceAccount -DisplayName 'Resource Timeline AMD service-profile qualification'
    Invoke-I2eSc -Arguments $createArgs | Out-Null
    $serviceCreated = $true
    $pointer.service_create_succeeded = $true
    $pointer.state = 'SERVICE_CREATED'
    Save-I2eExperimentPointer
    Invoke-I2eSc -Arguments @('sidtype', $ServiceName, 'unrestricted') | Out-Null
    $pointer.state = 'SERVICE_SID_CONFIGURED'
    Save-I2eExperimentPointer
    Assert-I2eServiceSidType -ServiceName $ServiceName
    $serviceSid = Resolve-I2eServiceSid -ServiceSidAccount $ServiceSidAccount
    $pointer.service_sid = $serviceSid
    $pointer.service_sid_resolved = $true
    $pointer.state = 'SERVICE_SID_RESOLVED'
    Save-I2eExperimentPointer
    Set-I2eDirectoryAcl -Path $QualificationRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $experimentRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $controlRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $treatmentRoot -ServiceSid $serviceSid
    Write-I2eJson -Path (Join-Path $experimentRoot 'AMD-CLI-PREFLIGHT.json') -Value $amdCliPreflight
    $pointer.state = 'READY_FOR_CONTROL'
    Save-I2eExperimentPointer

    $baseline = Get-I2eDirectAccountRightsSnapshot -Label 'dedicated-service-sid-before' -Sid $serviceSid
    $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($baseline.status -ne 'READ' -or $assigned.status -ne 'READ') { throw 'I2E baseline rights could not be read.' }
    $rightPresentBefore = @($baseline.direct_rights) -contains $I2eRequiredRight
    $rightAssignedBefore = @($assigned.assigned_principals) -contains $serviceSid
    if ($rightPresentBefore -or $rightAssignedBefore) {
        throw 'Dedicated Service SID already has SeSystemProfilePrivilege; refusing unknown pre-existing assignment.'
    }
    $plan = Get-I2eExperimentPlan -ArtifactSha256 $artifactHash
    $plan.experiment_id = $experimentId
    $plan.service_sid = $serviceSid
    $plan.control_scope = $controlScope
    $plan.treatment_scope = $treatmentScope
    $plan.baseline_direct_rights = $baseline.direct_rights
    $plan.baseline_account_object_state = $baseline.account_object_state
    $plan.baseline_direct_rights_status = $baseline.status
    $plan.baseline_right_assignment = $assigned
    $plan.se_system_profile_privilege_present_before = $rightPresentBefore
    $plan.right_was_present_before = $rightPresentBefore -or $rightAssignedBefore
    Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-PLAN.json') -Value $plan

    $pointer.control_execution_state = 'STARTING'
    $pointer.paired_gate_consumed = $true
    $pointer.state = 'CONTROL_EXECUTING'
    Save-I2eExperimentPointer
    $control = Invoke-I2ePhase -Phase CONTROL -Scope $controlScope -OutputRoot $controlRoot -ServiceSid $serviceSid
    $pointer.control_execution_state = 'COMPLETED'
    $pointer.control_executed = $true
    $pointer.control_result = [string]$control.summary.availability
    $pointer.state = 'CONTROL_EXECUTED'
    Save-I2eExperimentPointer
    Write-I2eJson -Path (Join-Path $experimentRoot 'CONTROL-RESULT.json') -Value $control.summary
    if ([string]$control.summary.availability -cne 'POWER_UNAVAILABLE') {
        throw ('Control result was {0}; treatment is not authorized.' -f $control.summary.availability)
    }

    $currentAmdCliPreflight = [pscustomobject]@{ preflight_pass = $false }
    $amdCliPreflightError = $null
    try {
        $currentAmdCliPreflight = Get-I2eAmdCliPreflight
    }
    catch {
        $amdCliPreflightError = $_.Exception.Message
    }
    $amdCliComparison = Compare-I2eAmdCliPreflight -Control $amdCliPreflight -Current $currentAmdCliPreflight
    $amdCliRevalidationEvidence = [ordered]@{
        schema = 'amd-service-profile-treatment-amd-cli-preflight/v1'
        qualification_only = $true
        experiment_id = $experimentId
        control_preflight_path = (Join-Path $experimentRoot 'AMD-CLI-PREFLIGHT.json')
        control_identity = $amdCliPreflight
        current_identity = $currentAmdCliPreflight
        comparison = $amdCliComparison.comparison
        differing_fields = @($amdCliComparison.differing_fields)
        pass = ($null -eq $amdCliPreflightError -and [bool]$amdCliComparison.pass)
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    if ($null -ne $amdCliPreflightError) { $amdCliRevalidationEvidence.error = $amdCliPreflightError }
    Write-I2eJson -Path (Join-Path $experimentRoot 'TREATMENT-AMD-CLI-PREFLIGHT.json') -Value $amdCliRevalidationEvidence
    if (-not $amdCliRevalidationEvidence.pass) {
        $details = if ($null -ne $amdCliPreflightError) { $amdCliPreflightError } else { ($amdCliComparison.differing_fields -join ', ') }
        throw ('AMD CLI identity changed between CONTROL and TREATMENT; refusing LSA mutation. Differences: {0}' -f $details)
    }

    $pointer.right_mutation_state = 'STARTING'
    $pointer.state = 'RIGHT_MUTATION_PENDING'
    Save-I2eExperimentPointer
    Add-I2eExactServiceProfileRight -ServiceSid $serviceSid
    $rightAdded = $true
    $pointer.right_added_by_experiment = $true
    $pointer.right_mutation_state = 'COMPLETED'
    $pointer.state = 'RIGHT_MUTATED'
    Save-I2eExperimentPointer
    $afterAdd = Get-I2eDirectAccountRightsSnapshot -Label 'dedicated-service-sid-after-add' -Sid $serviceSid
    $assignmentAfterAdd = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($afterAdd.status -ne 'READ' -or $assignmentAfterAdd.status -ne 'READ' -or
        @($afterAdd.direct_rights) -notcontains $I2eRequiredRight -or
        @($assignmentAfterAdd.assigned_principals) -notcontains $serviceSid) {
        throw 'Exact service-SID right was not verified after mutation.'
    }
    Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-APPLIED.json') -Value ([ordered]@{
        schema = 'amd-service-profile-security-mutation-applied/v1'
        qualification_only = $true
        experiment_id = $experimentId
        service_name = $ServiceName
        service_sid = $serviceSid
        right = $I2eRequiredRight
        right_was_present_before = $rightPresentBefore -or $rightAssignedBefore
        right_added_by_experiment = $true
        applied_at_utc = [DateTime]::UtcNow.ToString('o')
        direct_verification = $afterAdd
        assignment_verification = $assignmentAfterAdd
    })

    $pointer.treatment_execution_state = 'STARTING'
    $pointer.state = 'TREATMENT_EXECUTING'
    Save-I2eExperimentPointer
    $treatment = Invoke-I2ePhase -Phase TREATMENT -Scope $treatmentScope -OutputRoot $treatmentRoot -ServiceSid $serviceSid
    $pointer.treatment_execution_state = 'COMPLETED'
    $pointer.treatment_executed = $true
    $pointer.treatment_result = [string]$treatment.summary.availability
    $pointer.state = 'TREATMENT_EXECUTED'
    Save-I2eExperimentPointer
    Write-I2eJson -Path (Join-Path $experimentRoot 'TREATMENT-RESULT.json') -Value $treatment.summary
    $tokenDelta = Compare-I2eTokenDelta -ControlContext $control.context -TreatmentContext $treatment.context
    Write-I2eJson -Path (Join-Path $experimentRoot 'TOKEN-DELTA.json') -Value $tokenDelta
    if (-not $tokenDelta.pass) { throw 'Treatment token delta was not exactly SeSystemProfilePrivilege.' }
    $pointer.control_result = [string]$control.summary.availability
    $pointer.treatment_result = [string]$treatment.summary.availability
    $pointer.token_delta_verified = $true
    Save-I2eExperimentPointer
    Write-Host ('I2E paired experiment completed; evidence root: {0}' -f $experimentRoot)
}
catch {
    $primaryError = $_.Exception
}
finally {
    try {
        $stopResult = $null
        $stopError = $null
        $serviceStopAttempted = [bool]$serviceCreated
        $serviceStopVerified = -not $serviceCreated
        $serviceStateAfterStop = if ($serviceCreated) { 'UNKNOWN' } else { 'ABSENT' }
        $servicePidAfterStop = if ($serviceCreated) { -1L } else { 0L }
        if ($serviceCreated) {
            try {
                $stopResult = Stop-I2eService -ServiceName $ServiceName
            }
            catch {
                $stopError = $_.Exception.Message
            }
        }
        $serviceAfterStop = Get-I2eServiceSnapshot -ServiceName $ServiceName
        $serviceStateAfterStop = if ($serviceAfterStop.present) { [string]$serviceAfterStop.state } else { 'ABSENT' }
        $servicePidAfterStop = if ($serviceAfterStop.present) { [int64]$serviceAfterStop.process_id } else { 0L }
        $serviceStopVerified = -not $serviceAfterStop.present -or
            ($serviceStateAfterStop -ceq 'Stopped' -and $servicePidAfterStop -eq 0)
        $ownedBrokerCountAfterStop = @(Get-I2eOwnedBrokerProcesses).Count
        $amdCliCountAfterStop = @(Get-I2eOwnedAmdProcesses -ExpectedAmdCliPath ([string]$amdCliPreflight.path)).Count

        $directVerification = $null
        $assignmentVerification = $null
        $policyRemoveAttempted = $false
        $policyRollbackVerified = -not $rightAdded
        $policyError = $null
        if ($rightAdded -and $null -ne $serviceSid) {
            $policyRemoveAttempted = $true
            try {
                Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
                $directVerification = Get-I2eDirectAccountRightsSnapshot -Label 'dedicated-service-sid-after-rollback' -Sid $serviceSid
                $assignmentVerification = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
                $policyRollbackVerified = $directVerification.status -eq 'READ' -and
                    $assignmentVerification.status -eq 'READ' -and
                    @($directVerification.direct_rights) -notcontains $I2eRequiredRight -and
                    @($assignmentVerification.assigned_principals) -notcontains $serviceSid
                if (-not $policyRollbackVerified) {
                    $policyError = 'Exact service-SID right rollback was not verified in both LSA readback directions.'
                }
            }
            catch {
                $policyError = $_.Exception.Message
                $policyRollbackVerified = $false
            }
        }

        $verification = Get-I2eRollbackVerification `
            -PolicyRollbackVerified $policyRollbackVerified `
            -ServicePresent $serviceAfterStop.present `
            -ServiceState $serviceStateAfterStop `
            -ServiceProcessId $servicePidAfterStop `
            -OwnedBrokerProcessCount $ownedBrokerCountAfterStop `
            -AmdCliProcessCount $amdCliCountAfterStop
        $effectiveTokenTeardownVerified = [bool]$verification.effective_token_teardown_verified
        $fullRollbackVerified = [bool]$verification.full_rollback_verified
        $rollbackVerified = $fullRollbackVerified
        $serviceRemoved = -not $serviceAfterStop.present
        $serviceRemovalError = $null

        $rollbackEvidence = [ordered]@{
            schema = 'amd-service-profile-security-mutation-rollback/v1'
            qualification_only = $true
            experiment_id = $experimentId
            service_name = $ServiceName
            service_sid = $serviceSid
            right = $I2eRequiredRight
            right_added_by_experiment = $rightAdded
            all_rights = $false
            service_stop_attempted = $serviceStopAttempted
            service_stop_verified = $serviceStopVerified
            service_state_after_stop = $serviceStateAfterStop
            service_pid_after_stop = $servicePidAfterStop
            owned_broker_process_count_after_stop = $ownedBrokerCountAfterStop
            amd_cli_process_count_after_stop = $amdCliCountAfterStop
            policy_remove_attempted = $policyRemoveAttempted
            policy_rollback_verified = $policyRollbackVerified
            effective_token_teardown_verified = $effectiveTokenTeardownVerified
            full_rollback_verified = $fullRollbackVerified
            service_registration_removed = $serviceRemoved
            rollback_at_utc = [DateTime]::UtcNow.ToString('o')
            rollback_verified = $fullRollbackVerified
            direct_verification = $directVerification
            assignment_verification = $assignmentVerification
        }
        if ($null -ne $stopError) { $rollbackEvidence.stop_error = $stopError }
        if ($null -ne $policyError) { $rollbackEvidence.policy_error = $policyError }
        Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value $rollbackEvidence

        if ($fullRollbackVerified -and $serviceCreated -and $serviceAfterStop.present) {
            try {
                Remove-I2eService -ServiceName $ServiceName
                $serviceRemoved = -not (Get-I2eServiceSnapshot -ServiceName $ServiceName).present
            }
            catch {
                $serviceRemovalError = $_.Exception.Message
                $cleanupError = $_.Exception
            }
        }
        if ($null -ne $serviceRemovalError) { $rollbackEvidence.service_removal_error = $serviceRemovalError }
        $rollbackEvidence.service_registration_removed = $serviceRemoved
        Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value $rollbackEvidence
        if ($serviceCreated -and (-not $fullRollbackVerified -or -not $serviceRemoved)) {
            if ($null -eq $cleanupError) {
                $cleanupError = [Exception]::new('I2E cleanup did not prove full rollback and service removal; CURRENT pointer retained.')
            }
        }

        $pointer.policy_rollback_verified = $policyRollbackVerified
        $pointer.effective_token_teardown_verified = $effectiveTokenTeardownVerified
        $pointer.full_rollback_verified = $fullRollbackVerified
        $pointer.service_stop_attempted = $serviceStopAttempted
        $pointer.service_stop_verified = $serviceStopVerified
        $pointer.service_state_after_stop = $serviceStateAfterStop
        $pointer.service_pid_after_stop = $servicePidAfterStop
        $pointer.owned_broker_process_count_after_stop = $ownedBrokerCountAfterStop
        $pointer.amd_cli_process_count_after_stop = $amdCliCountAfterStop
        $pointer.service_registration_removed = $serviceRemoved
        $pointer.rollback_verified = $rollbackVerified
        if ($fullRollbackVerified -and $serviceRemoved) { $pointer.state = 'ROLLBACK_COMPLETE' }
        elseif ($policyRollbackVerified) { $pointer.state = 'POLICY_ROLLBACK_COMPLETE' }
        else { $pointer.state = 'SERVICE_STOP_OR_POLICY_ROLLBACK_PENDING' }
        Save-I2eExperimentPointer
    }
    catch {
        $cleanupError = $_.Exception
    }
}

if ($null -ne $cleanupError) { throw ('I2E cleanup failed closed: {0}' -f $cleanupError.Message) }
if ($null -ne $primaryError) { throw $primaryError }
