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

function Set-I2eObjectProperty {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][object]$Value
    )

    $property = @(
        $Object.PSObject.Properties |
            Where-Object Name -eq $Name |
            Select-Object -First 1
    )
    if ($property.Count -eq 1) {
        $property[0].Value = $Value
    }
    else {
        $Object | Add-Member -MemberType NoteProperty -Name $Name -Value $Value
    }
    return $Object
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

function Get-I2ePolicyRollbackState {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][object]$Pointer)

    $policyProperty = @($Pointer.PSObject.Properties | Where-Object Name -eq 'policy_rollback_verified' | Select-Object -First 1)
    if ($policyProperty.Count -eq 1) {
        return [pscustomobject]@{
            field_present = $true
            verified = [bool]$policyProperty[0].Value
            source = 'policy_rollback_verified'
        }
    }

    # Historical pointers predate the split rollback state.  Their legacy
    # rollback_verified value is the only available conservative signal.
    [pscustomobject]@{
        field_present = $false
        verified = Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'rollback_verified'
        source = 'legacy_rollback_verified_fallback'
    }
}

function Test-I2eExactRightRollbackRequired {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][object]$Pointer)

    if (-not (Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'right_added_by_experiment')) {
        return $false
    }
    return -not (Get-I2ePolicyRollbackState -Pointer $Pointer).verified
}

function Test-I2ePolicyRollbackStateDrift {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][object]$Pointer,
        [Parameter(Mandatory = $true)][bool]$ReadbackAvailable,
        [Parameter(Mandatory = $true)][bool]$RightPresent,
        [Parameter(Mandatory = $true)][bool]$AssignmentPresent
    )

    $state = Get-I2ePolicyRollbackState -Pointer $Pointer
    return $state.verified -and $ReadbackAvailable -and ($RightPresent -or $AssignmentPresent)
}

function Resolve-I2ePolicyRollbackDecision {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][object]$Pointer,
        [Parameter(Mandatory = $true)][bool]$ReadbackAvailable,
        [Parameter(Mandatory = $true)][bool]$RightPresent,
        [Parameter(Mandatory = $true)][bool]$AssignmentPresent
    )

    $rightAdded = Get-I2eCleanupPointerBoolean -Pointer $Pointer -Name 'right_added_by_experiment'
    $policyState = Get-I2ePolicyRollbackState -Pointer $Pointer
    $policyPresent = $RightPresent -or $AssignmentPresent

    if (-not $rightAdded) {
        return [pscustomobject]@{
            right_added_by_experiment = $false
            policy_rollback_required = $false
            policy_remove_allowed = $false
            policy_rollback_verified = $true
            policy_readback_required = $false
            policy_already_absent = $false
            policy_state_drift = $false
            fail_closed = $false
            policy_remove_skipped_reason = 'NO_RIGHT_ADDED'
        }
    }

    if (-not $ReadbackAvailable) {
        return [pscustomobject]@{
            right_added_by_experiment = $true
            policy_rollback_required = -not $policyState.verified
            policy_remove_allowed = $false
            policy_rollback_verified = $false
            policy_readback_required = $true
            policy_already_absent = $false
            policy_state_drift = $false
            fail_closed = $true
            policy_remove_skipped_reason = 'READBACK_UNAVAILABLE'
        }
    }

    if (-not $policyPresent) {
        return [pscustomobject]@{
            right_added_by_experiment = $true
            policy_rollback_required = $false
            policy_remove_allowed = $false
            policy_rollback_verified = $true
            policy_readback_required = $true
            policy_already_absent = $true
            policy_state_drift = $false
            fail_closed = $false
            policy_remove_skipped_reason = if ($policyState.verified) {
                'ALREADY_VERIFIED_REMOVED'
            }
            else {
                'POLICY_ALREADY_ABSENT_ON_RECOVERY'
            }
        }
    }

    if ($policyState.verified) {
        return [pscustomobject]@{
            right_added_by_experiment = $true
            policy_rollback_required = $false
            policy_remove_allowed = $false
            policy_rollback_verified = $false
            policy_readback_required = $true
            policy_already_absent = $false
            policy_state_drift = $true
            fail_closed = $true
            policy_remove_skipped_reason = 'POLICY_ROLLBACK_STATE_DRIFT'
        }
    }

    [pscustomobject]@{
        right_added_by_experiment = $true
        policy_rollback_required = $true
        policy_remove_allowed = $true
        policy_rollback_verified = $false
        policy_readback_required = $true
        policy_already_absent = $false
        policy_state_drift = $false
        fail_closed = $false
        policy_remove_skipped_reason = $null
    }
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
