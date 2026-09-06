#requires -Version 5.1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-I2eResumeProperty {
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

function Test-I2eResumeExactValue {
    param(
        [AllowNull()][object]$Actual,
        [AllowNull()][object]$Expected
    )

    if ($null -eq $Actual -or $null -eq $Expected) { return $Actual -eq $Expected }
    return ([string]$Actual).Equals([string]$Expected, [StringComparison]::OrdinalIgnoreCase)
}

function Test-I2eResumeBoolean {
    param(
        [AllowNull()][object]$Actual,
        [bool]$Expected
    )

    return $null -ne $Actual -and ([bool]$Actual -eq $Expected)
}

function Test-I2eResumeFixedArguments {
    param([AllowNull()][object]$Arguments)

    $values = @($Arguments | ForEach-Object { [string]$_ })
    return $values.Count -eq 2 -and $values[0] -ceq 'timechart' -and $values[1] -ceq '--list'
}

function Assert-I2eControlRecoveryEvidence {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]$Pointer,
        [Parameter(Mandatory = $true)]$ControlTokenGate,
        [Parameter(Mandatory = $true)]$ControlContext,
        [Parameter(Mandatory = $true)]$ControlSummary,
        [Parameter(Mandatory = $true)]$ControlDiscoveryResult,
        [Parameter(Mandatory = $true)]$ControlLaunch,
        [AllowNull()]$ControlHarnessError,
        [Parameter(Mandatory = $true)][string]$ExpectedExperimentId,
        [Parameter(Mandatory = $true)][string]$ExpectedControlScope,
        [Parameter(Mandatory = $true)][string]$ExpectedTreatmentScope,
        [Parameter(Mandatory = $true)][string]$ExpectedArtifactSha256,
        [Parameter(Mandatory = $true)][string]$ExpectedServiceSid,
        [Parameter(Mandatory = $true)][string]$ExpectedServiceName
    )

    $failures = @()
    foreach ($check in @(
        @{ Name = 'experiment_id'; Actual = (Get-I2eResumeProperty $Pointer 'experiment_id'); Expected = $ExpectedExperimentId },
        @{ Name = 'control_scope'; Actual = (Get-I2eResumeProperty $Pointer 'control_scope'); Expected = $ExpectedControlScope },
        @{ Name = 'treatment_scope'; Actual = (Get-I2eResumeProperty $Pointer 'treatment_scope'); Expected = $ExpectedTreatmentScope },
        @{ Name = 'artifact_sha256'; Actual = (Get-I2eResumeProperty $Pointer 'artifact_sha256'); Expected = $ExpectedArtifactSha256 },
        @{ Name = 'service_name'; Actual = (Get-I2eResumeProperty $Pointer 'service_name'); Expected = $ExpectedServiceName },
        @{ Name = 'service_sid'; Actual = (Get-I2eResumeProperty $Pointer 'service_sid'); Expected = $ExpectedServiceSid }
    )) {
        if (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "pointer.$($check.Name)"
        }
    }

    if (-not (Test-I2eResumeBoolean (Get-I2eResumeProperty $Pointer 'paired_gate_consumed') $true)) {
        $failures += 'pointer.paired_gate_consumed'
    }
    $controlExecutionState = [string](Get-I2eResumeProperty $Pointer 'control_execution_state' -Default '')
    $controlWasRecovered = (Test-I2eResumeExactValue $controlExecutionState 'COMPLETED_RECOVERED') -and
        (Test-I2eResumeBoolean (Get-I2eResumeProperty $Pointer 'control_executed') $true) -and
        (Test-I2eResumeExactValue (Get-I2eResumeProperty $Pointer 'control_result') 'POWER_UNAVAILABLE')
    $controlIsStale = (Test-I2eResumeExactValue (Get-I2eResumeProperty $Pointer 'state') 'ROLLBACK_COMPLETE') -and
        (Test-I2eResumeExactValue $controlExecutionState 'STARTING') -and
        (Test-I2eResumeBoolean (Get-I2eResumeProperty $Pointer 'control_executed') $false) -and
        $null -eq (Get-I2eResumeProperty $Pointer 'control_result')
    if (-not $controlWasRecovered -and -not $controlIsStale) {
        $failures += 'pointer.control_recovery_state'
    }
    if (-not (Test-I2eResumeExactValue (Get-I2eResumeProperty $Pointer 'right_mutation_state' -Default '') 'NOT_STARTED')) {
        $failures += 'pointer.right_mutation_state'
    }
    if (-not (Test-I2eResumeExactValue (Get-I2eResumeProperty $Pointer 'treatment_execution_state' -Default '') 'NOT_STARTED')) {
        $failures += 'pointer.treatment_execution_state'
    }
    if (Test-I2eResumeBoolean (Get-I2eResumeProperty $Pointer 'right_added_by_experiment') $true) {
        $failures += 'pointer.right_added_by_experiment'
    }
    if (Test-I2eResumeBoolean (Get-I2eResumeProperty $Pointer 'treatment_executed') $true) {
        $failures += 'pointer.treatment_executed'
    }

    foreach ($check in @(
        @{ Name = 'gate_pass'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'gate_pass'); Expected = $true },
        @{ Name = 'account_sid'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'account_sid'); Expected = 'S-1-5-19' },
        @{ Name = 'service_sid'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'service_sid'); Expected = $ExpectedServiceSid },
        @{ Name = 'session_id'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'session_id'); Expected = 0 },
        @{ Name = 'process_architecture'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'process_architecture'); Expected = 'x64' },
        @{ Name = 'administrators_sid_present'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'administrators_sid_present'); Expected = $false },
        @{ Name = 'se_system_profile_privilege_present'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'se_system_profile_privilege_present'); Expected = $false },
        @{ Name = 'se_system_profile_privilege_enabled'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'se_system_profile_privilege_enabled'); Expected = $false },
        @{ Name = 'se_system_profile_privilege_disabled'; Actual = (Get-I2eResumeProperty $ControlTokenGate 'se_system_profile_privilege_disabled'); Expected = $false }
    )) {
        if ($check.Expected -is [bool]) {
            if (-not (Test-I2eResumeBoolean $check.Actual $check.Expected)) { $failures += "token_gate.$($check.Name)" }
        } elseif (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "token_gate.$($check.Name)"
        }
    }
    if (@(Get-I2eResumeProperty $ControlTokenGate 'forbidden_enabled_privileges' -Default @()).Count -ne 0) {
        $failures += 'token_gate.forbidden_enabled_privileges'
    }

    foreach ($check in @(
        @{ Name = 'account_sid'; Actual = (Get-I2eResumeProperty $ControlContext 'account_sid'); Expected = 'S-1-5-19' },
        @{ Name = 'service_sid'; Actual = (Get-I2eResumeProperty $ControlContext 'service_sid'); Expected = $ExpectedServiceSid },
        @{ Name = 'session_id'; Actual = (Get-I2eResumeProperty $ControlContext 'session_id'); Expected = 0 },
        @{ Name = 'process_architecture'; Actual = (Get-I2eResumeProperty $ControlContext 'process_architecture'); Expected = 'x64' },
        @{ Name = 'context_valid'; Actual = (Get-I2eResumeProperty $ControlContext 'context_valid'); Expected = $true }
    )) {
        if ($check.Expected -is [bool]) {
            if (-not (Test-I2eResumeBoolean $check.Actual $check.Expected)) { $failures += "context.$($check.Name)" }
        } elseif (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "context.$($check.Name)"
        }
    }

    foreach ($check in @(
        @{ Name = 'service_name'; Actual = (Get-I2eResumeProperty $ControlSummary 'service_name'); Expected = $ExpectedServiceName },
        @{ Name = 'service_account_sid'; Actual = (Get-I2eResumeProperty $ControlSummary 'service_account_sid'); Expected = 'S-1-5-19' },
        @{ Name = 'service_sid'; Actual = (Get-I2eResumeProperty $ControlSummary 'service_sid'); Expected = $ExpectedServiceSid },
        @{ Name = 'phase'; Actual = (Get-I2eResumeProperty $ControlSummary 'phase'); Expected = 'CONTROL' },
        @{ Name = 'availability'; Actual = (Get-I2eResumeProperty $ControlSummary 'availability'); Expected = 'POWER_UNAVAILABLE' },
        @{ Name = 'power_category_present'; Actual = (Get-I2eResumeProperty $ControlSummary 'power_category_present'); Expected = $false },
        @{ Name = 'no_orphan_child'; Actual = (Get-I2eResumeProperty $ControlSummary 'no_orphan_child'); Expected = $true },
        @{ Name = 'sampling'; Actual = (Get-I2eResumeProperty $ControlSummary 'sampling'); Expected = $false }
    )) {
        if ($check.Expected -is [bool]) {
            if (-not (Test-I2eResumeBoolean $check.Actual $check.Expected)) { $failures += "summary.$($check.Name)" }
        } elseif (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "summary.$($check.Name)"
        }
    }
    if ([int]$ControlSummary.cli_exit_code -ne 0) { $failures += 'summary.cli_exit_code' }
    if (-not (Test-I2eResumeFixedArguments (Get-I2eResumeProperty $ControlSummary 'fixed_cli_arguments'))) {
        $failures += 'summary.fixed_cli_arguments'
    }

    foreach ($check in @(
        @{ Name = 'availability'; Actual = (Get-I2eResumeProperty $ControlDiscoveryResult 'availability'); Expected = 'POWER_UNAVAILABLE' },
        @{ Name = 'power_category_present'; Actual = (Get-I2eResumeProperty $ControlDiscoveryResult 'power_category_present'); Expected = $false },
        @{ Name = 'no_counters_available_diagnostic'; Actual = (Get-I2eResumeProperty $ControlDiscoveryResult 'no_counters_available_diagnostic'); Expected = $true },
        @{ Name = 'no_orphan_child'; Actual = (Get-I2eResumeProperty $ControlDiscoveryResult 'no_orphan_child'); Expected = $true },
        @{ Name = 'sampling'; Actual = (Get-I2eResumeProperty $ControlDiscoveryResult 'sampling'); Expected = $false }
    )) {
        if ($check.Expected -is [bool]) {
            if (-not (Test-I2eResumeBoolean $check.Actual $check.Expected)) { $failures += "discovery.$($check.Name)" }
        } elseif (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "discovery.$($check.Name)"
        }
    }
    if ([int]$ControlDiscoveryResult.cli_exit_code -ne 0) { $failures += 'discovery.cli_exit_code' }
    if (-not (Test-I2eResumeFixedArguments (Get-I2eResumeProperty $ControlDiscoveryResult 'arguments'))) {
        $failures += 'discovery.arguments'
    }

    if ($null -ne $ControlHarnessError) { $failures += 'control_harness_error' }
    if (-not (Test-I2eResumeBoolean (Get-I2eResumeProperty $ControlLaunch 'counter_discovery_only') $true) -or
        -not (Test-I2eResumeBoolean (Get-I2eResumeProperty $ControlLaunch 'sampling') $false) -or
        -not (Test-I2eResumeFixedArguments (Get-I2eResumeProperty $ControlLaunch 'arguments'))) {
        $failures += 'launch.fixed_non_sampling_command'
    }

    if ($failures.Count -gt 0) {
        throw ('I2E control recovery evidence validation failed: {0}' -f ($failures -join ', '))
    }

    [ordered]@{
        pass = $true
        experiment_id = $ExpectedExperimentId
        control_scope = $ExpectedControlScope
        treatment_scope = $ExpectedTreatmentScope
        artifact_sha256 = $ExpectedArtifactSha256
        service_name = $ExpectedServiceName
        service_sid = $ExpectedServiceSid
        control_real_executed = $true
        control_result = 'POWER_UNAVAILABLE'
        paired_gate_consumed = $true
        control_recovery_required = $controlIsStale
        recovery_reason = 'POWERSHELL_AUTOMATIC_VARIABLE_PID_COLLISION_AFTER_REAL_CONTROL'
    }
}

function Assert-I2eTreatmentTokenGate {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]$TokenGate,
        [Parameter(Mandatory = $true)]$Context,
        [Parameter(Mandatory = $true)][string]$ExpectedServiceSid
    )

    $failures = @()
    foreach ($check in @(
        @{ Name = 'gate_pass'; Actual = (Get-I2eResumeProperty $TokenGate 'gate_pass'); Expected = $true },
        @{ Name = 'account_sid'; Actual = (Get-I2eResumeProperty $TokenGate 'account_sid'); Expected = 'S-1-5-19' },
        @{ Name = 'service_sid'; Actual = (Get-I2eResumeProperty $TokenGate 'service_sid'); Expected = $ExpectedServiceSid },
        @{ Name = 'session_id'; Actual = (Get-I2eResumeProperty $TokenGate 'session_id'); Expected = 0 },
        @{ Name = 'process_architecture'; Actual = (Get-I2eResumeProperty $TokenGate 'process_architecture'); Expected = 'x64' },
        @{ Name = 'administrators_sid_present'; Actual = (Get-I2eResumeProperty $TokenGate 'administrators_sid_present'); Expected = $false },
        @{ Name = 'se_system_profile_privilege_present'; Actual = (Get-I2eResumeProperty $TokenGate 'se_system_profile_privilege_present'); Expected = $true },
        @{ Name = 'se_system_profile_privilege_enabled'; Actual = (Get-I2eResumeProperty $TokenGate 'se_system_profile_privilege_enabled'); Expected = $true },
        @{ Name = 'se_system_profile_privilege_disabled'; Actual = (Get-I2eResumeProperty $TokenGate 'se_system_profile_privilege_disabled'); Expected = $false },
        @{ Name = 'se_profile_single_process_privilege_newly_introduced'; Actual = (Get-I2eResumeProperty $TokenGate 'se_profile_single_process_privilege_newly_introduced'); Expected = $false },
        @{ Name = 'se_debug_privilege_newly_introduced'; Actual = (Get-I2eResumeProperty $TokenGate 'se_debug_privilege_newly_introduced'); Expected = $false }
    )) {
        if ($check.Expected -is [bool]) {
            if (-not (Test-I2eResumeBoolean $check.Actual $check.Expected)) { $failures += "token_gate.$($check.Name)" }
        } elseif (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "token_gate.$($check.Name)"
        }
    }
    if (@(Get-I2eResumeProperty $TokenGate 'forbidden_enabled_privileges' -Default @()).Count -ne 0) {
        $failures += 'token_gate.forbidden_enabled_privileges'
    }
    foreach ($check in @(
        @{ Name = 'account_sid'; Actual = (Get-I2eResumeProperty $Context 'account_sid'); Expected = 'S-1-5-19' },
        @{ Name = 'service_sid'; Actual = (Get-I2eResumeProperty $Context 'service_sid'); Expected = $ExpectedServiceSid },
        @{ Name = 'session_id'; Actual = (Get-I2eResumeProperty $Context 'session_id'); Expected = 0 },
        @{ Name = 'process_architecture'; Actual = (Get-I2eResumeProperty $Context 'process_architecture'); Expected = 'x64' },
        @{ Name = 'context_valid'; Actual = (Get-I2eResumeProperty $Context 'context_valid'); Expected = $true }
    )) {
        if ($check.Expected -is [bool]) {
            if (-not (Test-I2eResumeBoolean $check.Actual $check.Expected)) { $failures += "context.$($check.Name)" }
        } elseif (-not (Test-I2eResumeExactValue $check.Actual $check.Expected)) {
            $failures += "context.$($check.Name)"
        }
    }
    if ($failures.Count -gt 0) {
        throw ('I2E treatment token gate failed: {0}' -f ($failures -join ', '))
    }
    $true
}
