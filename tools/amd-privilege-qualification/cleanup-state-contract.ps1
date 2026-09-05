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
