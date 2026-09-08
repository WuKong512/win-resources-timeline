#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# This file is a pure contract library.  Dot-sourcing it must not inspect the
# host, resolve an account, create a scope, read evidence, or invoke a process.
$I2gHarnessImplemented = $true
$I2gVariable = 'SeProfileSingleProcessPrivilege'
$I2gSelectionConfidence = 'MEDIUM'
$I2gExperimentShape = 'PAIRED_CONTROL_TREATMENT'
$I2gHistoricalI2fRole = 'PREDECESSOR_EVIDENCE_ONLY'
$I2gHistoricalI2fIsActiveCausalControl = $false
$I2gRealGateConsumed = $false
$I2gRealExecutionAllowed = $false
$I2gRealCleanupAllowed = $false
$I2gHumanAuthorizationRecorded = $false
$I2gQualificationOnly = $true

$I2gServiceName = 'ResourceTimelineAmdProfileSingleProcessQualification'
$I2gServiceAccount = 'NT AUTHORITY\LocalService'
$I2gServiceAccountSid = 'S-1-5-19'
$I2gServiceSidType = 'UNRESTRICTED'
$I2gFixedAmdCliPath = 'D:\apps\AMDuProf\bin\AMDuProfCLI.exe'
$I2gFixedAmdCliSha256 = 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC'
$I2gFixedAmdCliVersion = '5.3.521.0'
$I2gFixedAmdCliArchitecture = 'x64'
$I2gFixedArguments = @('timechart', '--list')
$I2gOperation = 'COUNTER_DISCOVERY'
$I2gCounterDiscoveryTimeoutMs = 30000
$I2gChildSafetyCapMs = 90000
$I2gPowerSamplingRuns = 0
$I2gPlannedControlRuns = 1
$I2gPlannedTreatmentRuns = 1
$I2gPlannedValidPairRuns = 2
$I2gMaxControlRuns = 1
$I2gMaxTreatmentRuns = 1
$I2gMaxTotalRuns = 2

$I2gControlPolicyRights = @('SeSystemProfilePrivilege')
$I2gTreatmentPolicyRights = @('SeSystemProfilePrivilege', 'SeProfileSingleProcessPrivilege')
$I2gTreatmentDelta = 'SeProfileSingleProcessPrivilege assignment to same Service SID only'
$I2gControlExpectedResult = 'POWER_UNAVAILABLE'
$I2gNoCodeChangeBetweenPhases = $true
$I2gNoHarnessRebuildBetweenPhases = $true
$I2gNoNonTreatmentConfigurationChangeBetweenPhases = $true
$I2gControlDriftStopsBeforeTreatment = $true
$I2gTokenTeardownBeforeTreatmentPolicyMutation = $true
$I2gTokenTeardownBeforePolicyRightRemoval = $true
$I2gNoRetry = $true
$I2gHarnessArtifactRelativePath = 'target\release\amd-privilege-qualification.exe'
$I2gHarnessArtifactArchitecture = 'x64'
# Replaced with the SHA-256 of the rebuilt release artifact before the wrapper is shipped.
$I2gHarnessArtifactSha256 = '2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80'

$I2gRequiredEvidenceFiles = @(
    'EXPERIMENT-MANIFEST.json',
    'PRE-MACHINE-STATE.json',
    'CONTROL-POLICY.json',
    'CONTROL-TOKEN-PRE.json',
    'CONTROL-TOKEN-POST.json',
    'CONTROL-AMD-IDENTITY.json',
    'CONTROL-DISCOVERY.json',
    'CONTROL-TEARDOWN.json',
    'TREATMENT-POLICY.json',
    'TREATMENT-TOKEN-PRE.json',
    'TREATMENT-TOKEN-SYSTEMPROFILE.json',
    'TREATMENT-TOKEN-FINAL.json',
    'TREATMENT-AMD-IDENTITY.json',
    'TREATMENT-DISCOVERY.json',
    'PAIRED-CONFIG-COMPARISON.json',
    'PAIRED-TOKEN-COMPARISON.json',
    'PAIRED-RESULT.json',
    'ROLLBACK.json',
    'FINAL-SUMMARY.json'
)

$I2gResultClasses = @(
    'CONTROL_DRIFT',
    'PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT',
    'PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT',
    'TOKEN_GATE_FAILED',
    'IDENTITY_MISMATCH',
    'INVALID_CONFIGURATION_DELTA',
    'INVALID_TOKEN_DELTA',
    'DISCOVERY_FAILED',
    'TIMEOUT',
    'PROCESS_OWNERSHIP_FAILED',
    'CLEANUP_FAILED',
    'INVALID_NO_CAUSAL_INTERPRETATION'
)

function Get-I2gExperimentPlan {
    [ordered]@{
        schema = 'amd-i2g-paired-experiment-plan/v1'
        qualification_only = $I2gQualificationOnly
        i2g_variable = $I2gVariable
        selection_confidence = $I2gSelectionConfidence
        experiment_shape = $I2gExperimentShape
        historical_i2f_role = $I2gHistoricalI2fRole
        historical_i2f_is_active_causal_control = $I2gHistoricalI2fIsActiveCausalControl
        service_name = $I2gServiceName
        service_account = $I2gServiceAccount
        service_account_sid = $I2gServiceAccountSid
        service_sid_type = $I2gServiceSidType
        fixed_amd_cli_path = $I2gFixedAmdCliPath
        fixed_amd_cli_sha256 = $I2gFixedAmdCliSha256
        fixed_amd_cli_version = $I2gFixedAmdCliVersion
        fixed_amd_cli_architecture = $I2gFixedAmdCliArchitecture
        fixed_cli_arguments = $I2gFixedArguments
        operation = $I2gOperation
        control_policy_rights = $I2gControlPolicyRights
        treatment_policy_rights = $I2gTreatmentPolicyRights
        treatment_delta = $I2gTreatmentDelta
        control_expected_result = $I2gControlExpectedResult
        sampling = $false
        power_sampling_runs = $I2gPowerSamplingRuns
        planned_control_runs = $I2gPlannedControlRuns
        planned_treatment_runs = $I2gPlannedTreatmentRuns
        planned_valid_pair_runs = $I2gPlannedValidPairRuns
        max_control_runs = $I2gMaxControlRuns
        max_treatment_runs = $I2gMaxTreatmentRuns
        max_total_runs = $I2gMaxTotalRuns
        retry_allowed = $false
        real_gate_consumed = $I2gRealGateConsumed
        real_execution_allowed = $I2gRealExecutionAllowed
        real_cleanup_allowed = $I2gRealCleanupAllowed
        human_authorization_recorded = $I2gHumanAuthorizationRecorded
        control_drift_stops_before_treatment = $I2gControlDriftStopsBeforeTreatment
        token_teardown_before_treatment_policy_mutation = $I2gTokenTeardownBeforeTreatmentPolicyMutation
        token_teardown_before_policy_right_removal = $I2gTokenTeardownBeforePolicyRightRemoval
        no_code_change_between_phases = $I2gNoCodeChangeBetweenPhases
        no_harness_rebuild_between_phases = $I2gNoHarnessRebuildBetweenPhases
        no_non_treatment_configuration_change_between_phases = $I2gNoNonTreatmentConfigurationChangeBetweenPhases
        harness_artifact_relative_path = $I2gHarnessArtifactRelativePath
        harness_artifact_architecture = $I2gHarnessArtifactArchitecture
        harness_artifact_sha256 = $I2gHarnessArtifactSha256
        i2g_execution_surface = 'SYNTHETIC_OFFLINE_FAIL_CLOSED'
        shared_executable_contains_historical_entrypoints = $true
    }
}
function Get-I2gRequiredEvidenceNames {
    @($I2gRequiredEvidenceFiles)
}

function Get-I2gHarnessArtifactArchitecture {
    param([Parameter(Mandatory = $true)][string]$Path)

    $stream = $null
    $reader = $null
    try {
        $stream = [System.IO.File]::OpenRead($Path)
        $reader = New-Object System.IO.BinaryReader($stream)
        if ($reader.ReadUInt16() -ne [uint16]0x5a4d) { return 'INVALID' }
        $stream.Position = 0x3c
        $peOffset = $reader.ReadInt32()
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne [uint32]0x00004550) { return 'INVALID' }
        switch ([uint16]$reader.ReadUInt16()) {
            ([uint16]0x8664) { return 'x64' }
            ([uint16]0x014c) { return 'x86' }
            default { return 'UNKNOWN' }
        }
    } catch {
        return 'INVALID'
    } finally {
        if ($null -ne $reader) { $reader.Dispose() }
        elseif ($null -ne $stream) { $stream.Dispose() }
    }
}

function Test-I2gHarnessArtifactIdentity {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$ExpectedPath
    )

    if ([string]::IsNullOrWhiteSpace($ExpectedPath)) {
        $ExpectedPath = Join-Path $PSScriptRoot $I2gHarnessArtifactRelativePath
    }
    $expectedPath = [System.IO.Path]::GetFullPath($ExpectedPath)
    $actualPath = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $actualPath -PathType Leaf)) {
        return [pscustomobject]@{
            pass = $false
            reason = 'MISSING_ARTIFACT'
            path = $actualPath
            expected_path = $expectedPath
            sha256 = $null
            architecture = $null
        }
    }
    if ($actualPath -cne $expectedPath) {
        return [pscustomobject]@{
            pass = $false
            reason = 'UNEXPECTED_ARTIFACT_PATH'
            path = $actualPath
            expected_path = $expectedPath
            sha256 = (Get-FileHash -LiteralPath $actualPath -Algorithm SHA256).Hash
            architecture = (Get-I2gHarnessArtifactArchitecture -Path $actualPath)
        }
    }
    $hash = (Get-FileHash -LiteralPath $actualPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $architecture = Get-I2gHarnessArtifactArchitecture -Path $actualPath
    $hashPass = $I2gHarnessArtifactSha256 -ne 'TO_BE_REBUILT' -and
        $hash -ceq $I2gHarnessArtifactSha256
    $architecturePass = $architecture -ceq $I2gHarnessArtifactArchitecture
    [pscustomobject]@{
        pass = $hashPass -and $architecturePass
        reason = if ($hashPass -and $architecturePass) { 'PASS' } elseif (-not $hashPass) { 'SHA256_MISMATCH' } else { 'ARCHITECTURE_MISMATCH' }
        path = $actualPath
        expected_path = $expectedPath
        sha256 = $hash
        expected_sha256 = $I2gHarnessArtifactSha256
        architecture = $architecture
        expected_architecture = $I2gHarnessArtifactArchitecture
    }
}

function Test-I2gFixedCliArguments {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    if ($Arguments.Count -ne $I2gFixedArguments.Count) { return $false }
    for ($index = 0; $index -lt $I2gFixedArguments.Count; $index++) {
        if ($Arguments[$index] -cne $I2gFixedArguments[$index]) { return $false }
    }
    $true
}

function Get-I2gScenarioExpectation {
    param([Parameter(Mandatory = $true)][ValidateNotNullOrEmpty()][string]$Scenario)
    switch ($Scenario.ToLowerInvariant().Replace('_', '-')) {
        'happy' { 'PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT'; break }
        'negative' { 'PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT'; break }
        'control-drift' { 'CONTROL_DRIFT'; break }
        'pre-control-failure' { 'TOKEN_GATE_FAILED'; break }
        'control-token-gate-failure' { 'TOKEN_GATE_FAILED'; break }
        'config-delta-failure' { 'INVALID_CONFIGURATION_DELTA'; break }
        'token-delta-failure' { 'INVALID_TOKEN_DELTA'; break }
        'materialization-failure' { 'TOKEN_GATE_FAILED'; break }
        'system-profile-regression' { 'TOKEN_GATE_FAILED'; break }
        'process-ownership-failure' { 'PROCESS_OWNERSHIP_FAILED'; break }
        'control-timeout' { 'TIMEOUT'; break }
        'treatment-timeout' { 'TIMEOUT'; break }
        'cleanup-failure' { 'CLEANUP_FAILED'; break }
        'identity-mismatch' { 'IDENTITY_MISMATCH'; break }
        'spawn-failure' { 'DISCOVERY_FAILED'; break }
        'exit-nonzero' { 'DISCOVERY_FAILED'; break }
        'unexpected-preexisting-profile-right' { 'INVALID_NO_CAUSAL_INTERPRETATION'; break }
        'recovery-matrix' { 'INVALID_NO_CAUSAL_INTERPRETATION'; break }
        'crash-window-matrix' { 'CRASH_RECOVERY_MATRIX'; break }
        default { throw "Unknown offline I2G scenario: $Scenario" }
    }
}
