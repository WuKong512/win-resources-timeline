# Pure cleanup decision seam.  It has no service, process, or filesystem side effects and is
# dot-sourced by the administrator cleanup wrapper and its synthetic regression tests.

function Resolve-QualificationStopDisposition {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][int]$StopExitCode,
        [Parameter(Mandatory = $true)][string]$ServiceState,
        [Parameter(Mandatory = $true)][int64]$ServiceProcessId,
        [Parameter(Mandatory = $true)][bool]$ServicePresent
    )

    if (-not $ServicePresent) {
        return 'SERVICE_ABSENT'
    }
    if ($ServiceState -eq 'Stopped' -and $ServiceProcessId -eq 0) {
        if ($StopExitCode -eq 0) {
            return 'SC_STOP_0_PROCEED_TO_DELETE'
        }
        if ($StopExitCode -eq 1062) {
            return 'SC_STOP_1062_PROCEED_TO_DELETE'
        }
        return 'SC_STOP_NONZERO_THEN_STOPPED_PID0_PROCEED_TO_DELETE'
    }
    return 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0'
}

function Get-I2eCleanupPointerField {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][object]$Pointer,
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][object]$Default = $null
    )

    $property = @($Pointer.PSObject.Properties | Where-Object Name -eq $Name | Select-Object -First 1)
    if ($property.Count -eq 1) { return $property[0].Value }
    return $Default
}

function Get-I2eCleanupPointerBoolean {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][object]$Pointer,
        [Parameter(Mandatory = $true)][string]$Name,
        [bool]$Default = $false
    )

    $value = Get-I2eCleanupPointerField -Pointer $Pointer -Name $Name -Default $Default
    if ($null -eq $value) { return $Default }
    return [bool]$value
}

function Resolve-I2eExperimentState {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][object]$Pointer)

    if (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'experiment_closed') {
        return 'CLOSED'
    }
    if (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'treatment_executed') {
        return 'TREATMENT_EXECUTED'
    }
    if (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'right_added_by_experiment') {
        return 'RIGHT_MUTATED'
    }
    if (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'control_executed') {
        return 'CONTROL_EXECUTED'
    }
    if (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'service_create_succeeded') {
        return 'SERVICE_CREATED'
    }
    return 'PRE_SERVICE_CREATE'
}

function Test-I2ePairedGateConsumed {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][object]$Pointer)

    $property = @($Pointer.PSObject.Properties | Where-Object Name -eq 'paired_gate_consumed' | Select-Object -First 1)
    if ($property.Count -eq 1) { return [bool]$property[0].Value }
    $state = Resolve-I2eExperimentState -Pointer $Pointer
    return $state -in @('CONTROL_EXECUTED', 'RIGHT_MUTATED', 'TREATMENT_EXECUTED')
}

function Test-I2eExactRightRollbackRequired {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][object]$Pointer)

    return (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'right_added_by_experiment') -and
        -not (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'rollback_verified')
}

function Get-I2eRollbackVerification {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][bool]$PolicyRollbackVerified,
        [Parameter(Mandatory = $true)][bool]$ServicePresent,
        [Parameter(Mandatory = $true)][string]$ServiceState,
        [Parameter(Mandatory = $true)][int64]$ServiceProcessId,
        [Parameter(Mandatory = $true)][int]$OwnedBrokerProcessCount,
        [Parameter(Mandatory = $true)][int]$AmdCliProcessCount
    )

    $serviceStopVerified = -not $ServicePresent -or
        ($ServiceState -ceq 'Stopped' -and $ServiceProcessId -eq 0)
    $effectiveTokenTeardownVerified = $serviceStopVerified -and
        $OwnedBrokerProcessCount -eq 0 -and
        $AmdCliProcessCount -eq 0
    [pscustomobject]@{
        policy_rollback_verified = $PolicyRollbackVerified
        service_stop_verified = $serviceStopVerified
        effective_token_teardown_verified = $effectiveTokenTeardownVerified
        full_rollback_verified = $PolicyRollbackVerified -and $effectiveTokenTeardownVerified
    }
}
