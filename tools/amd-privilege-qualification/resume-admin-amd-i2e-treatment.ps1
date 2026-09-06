#requires -Version 5.1
[CmdletBinding()]
param([switch]$ExecuteAuthorizedTreatmentOnly)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ResumeScriptRoot = $PSScriptRoot
$SetupScriptPath = Join-Path $ResumeScriptRoot 'run-admin-amd-i2e-service-profile-experiment.ps1'
. $SetupScriptPath -LibraryOnly
. (Join-Path $ResumeScriptRoot 'i2e-treatment-resume-contract.ps1')

$ExpectedExperimentId = '3935ac9082954bcfb2b1f94c54cf95d7'
$ExpectedControlScope = '07a511e169274def93da79f269792b71'
$ExpectedTreatmentScope = 'e66bbcff49ff4aeaaf8bd2a75aa959c7'
$ExpectedServiceSid = 'S-1-5-80-2365814672-2637389132-1660472602-1496836994-3411780124'
$ExpectedArtifactSha256 = '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9'
$ExpectedServiceStartMode = '--service-profile-counter-service'

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

function Get-I2eResumeAmdProcesses {
    @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and $_.Path -ieq 'D:\apps\AMDuProf\bin\AMDuProfCLI.exe' } catch { $false }
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
    Assert-I2eServiceSidType
    $resolvedSid = Resolve-I2eServiceSid
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
    Write-I2eJson -Path (Join-Path $experimentRoot 'CONTROL-RECOVERY.json') -Value ([ordered]@{
        schema = 'amd-service-profile-control-recovery/v1'
        qualification_only = $true
        experiment_id = $ExpectedExperimentId
        control_scope = $ExpectedControlScope
        original_pointer_state = [string](Get-I2eResumeProperty $Pointer 'state' -Default 'UNKNOWN')
        original_control_execution_state = [string](Get-I2eResumeProperty $Pointer 'control_execution_state' -Default 'UNKNOWN')
        recovery_reason = 'POWERSHELL_AUTOMATIC_VARIABLE_PID_COLLISION_AFTER_REAL_CONTROL'
        authoritative_sources = @(
            (Join-Path $ControlEvidence.control_root 'SERVICE-PROFILE-TOKEN-GATE.json'),
            (Join-Path $ControlEvidence.control_root 'SERVICE-PROFILE-SERVICE-CONTEXT.json'),
            (Join-Path $ControlEvidence.control_root 'SERVICE-PROFILE-COUNTER-SUMMARY.json'),
            (Join-Path $ControlEvidence.control_root 'AMD-COUNTER-DISCOVERY-RESULT.json'),
            (Join-Path $ControlEvidence.control_root 'AMD-COUNTER-DISCOVERY-LAUNCH.json')
        )
        control_real_executed = $true
        control_result = 'POWER_UNAVAILABLE'
        paired_gate_consumed = $true
        recovered_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    $Pointer.control_execution_state = 'COMPLETED_RECOVERED'
    $Pointer.control_executed = $true
    $Pointer.control_result = 'POWER_UNAVAILABLE'
    $Pointer.paired_gate_consumed = $true
    $Pointer.state = 'CONTROL_EXECUTED_RECOVERED'
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
    if (@(Get-I2eResumeOwnedBrokerProcesses).Count -ne 0) {
        throw 'Treatment resume found an owned qualification broker process.'
    }
    if (@(Get-I2eResumeAmdProcesses).Count -ne 0) {
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
        [Parameter(Mandatory = $true)][string]$ExperimentRoot
    )
    Remove-I2eExactServiceProfileRight -ServiceSid $ServiceSid
    $direct = Get-I2eDirectAccountRightsSnapshot -Label 'treatment-after-rollback' -Sid $ServiceSid
    $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($direct.status -ne 'READ' -or $assigned.status -ne 'READ' -or
        @($direct.direct_rights) -contains $I2eRequiredRight -or
        @($assigned.assigned_principals) -contains $ServiceSid) {
        throw 'Treatment exact-right rollback was not verified in both directions.'
    }
    Write-I2eJson -Path (Join-Path $ExperimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value ([ordered]@{
        schema = 'amd-service-profile-security-mutation-rollback/v1'
        qualification_only = $true
        experiment_id = $ExpectedExperimentId
        service_name = $ServiceName
        service_sid = $ServiceSid
        right = $I2eRequiredRight
        right_added_by_experiment = $true
        all_rights = $false
        rollback_at_utc = [DateTime]::UtcNow.ToString('o')
        rollback_verified = $true
        direct_verification = $direct
        assignment_verification = $assigned
    })
    $true
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
        service_account = $I2eServiceAccount
        service_account_sid = $I2eServiceAccountSid
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

if (-not $ExecuteAuthorizedTreatmentOnly) {
    Get-I2eTreatmentResumePlan | ConvertTo-Json -Depth 20
    Write-Host 'I2E_TREATMENT_RESUME_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, or AMD runtime was performed.'
    return
}

$null = Assert-I2eAdministrator
$artifactHash = Assert-I2eTreatmentResumeArtifact
$pointer = Assert-I2eTreatmentResumePointer
$serviceGate = Assert-I2eTreatmentResumeServiceGate
Assert-I2eTreatmentResumeNoOwnedProcesses
Assert-I2eTreatmentEvidenceAbsent
$controlEvidence = Get-I2eControlRecoveryEvidence -Pointer $pointer
$amdPreflight = Read-I2eResumeEvidence -Path (Join-Path (Join-Path $QualificationRoot $ExpectedExperimentId) 'AMD-CLI-PREFLIGHT.json')
if (-not (Test-I2eResumeBoolean (Get-I2eResumeProperty $amdPreflight 'preflight_pass') $true)) {
    throw 'Existing AMD CLI preflight did not pass; refusing treatment.'
}
$before = Assert-I2eTreatmentResumeSecurityGate -ServiceSid $ExpectedServiceSid
$experimentRoot = Join-Path $QualificationRoot $ExpectedExperimentId
Write-I2eControlRecovery -Pointer $pointer -ControlEvidence $controlEvidence
$pointer.right_mutation_state = 'STARTING'
$pointer.state = 'RIGHT_MUTATION_PENDING'
Write-I2eJson -Path $PointerPath -Value $pointer
$rightAdded = $false
$treatment = $null
$rollbackVerified = $false
$primaryError = $null
$cleanupError = $null
try {
    Add-I2eExactServiceProfileRight -ServiceSid $ExpectedServiceSid
    $rightAdded = $true
    $pointer.right_added_by_experiment = $true
    $pointer.right_mutation_state = 'COMPLETED'
    $pointer.state = 'RIGHT_MUTATED'
    Write-I2eJson -Path $PointerPath -Value $pointer
    $null = Write-I2eTreatmentMutationApplied -ServiceSid $ExpectedServiceSid -Before $before

    $pointer.treatment_execution_state = 'STARTING'
    $pointer.state = 'TREATMENT_EXECUTING'
    Write-I2eJson -Path $PointerPath -Value $pointer
    $treatment = Invoke-I2ePhase -Phase TREATMENT -Scope $ExpectedTreatmentScope -OutputRoot (Join-Path $QualificationRoot $ExpectedTreatmentScope) -ServiceSid $ExpectedServiceSid
    Assert-I2eTreatmentTokenGate -TokenGate $treatment.token_gate -Context $treatment.context -ExpectedServiceSid $ExpectedServiceSid
    $pointer.treatment_execution_state = 'COMPLETED'
    $pointer.treatment_executed = $true
    $pointer.treatment_result = [string]$treatment.summary.availability
    $pointer.state = 'TREATMENT_EXECUTED'
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
            $rollbackVerified = Invoke-I2eTreatmentRollback -ServiceSid $ExpectedServiceSid -ExperimentRoot $experimentRoot
            $pointer.rollback_verified = $rollbackVerified
            $pointer.right_mutation_state = 'ROLLED_BACK'
        }
        $pointer.rollback_verified = $rollbackVerified
        if ($rollbackVerified) { $pointer.state = 'ROLLBACK_COMPLETE' }
        Write-I2eJson -Path $PointerPath -Value $pointer
        Stop-I2eService | Out-Null
        Remove-I2eService
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
