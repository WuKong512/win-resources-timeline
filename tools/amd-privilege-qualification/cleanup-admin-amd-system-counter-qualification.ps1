#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cleanup-state-contract.ps1')

$ServiceName = 'ResourceTimelineAmdSystemCounterQualification'
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$QualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-counter'
$ConfigPath = Join-Path $QualificationRoot 'SYSTEM-CONFIG.json'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'SYSTEM comparison cleanup requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
}
if (-not [Environment]::Is64BitProcess) {
    throw 'SYSTEM comparison cleanup requires x64 PowerShell.'
}

$sc = Join-Path $env:SystemRoot 'System32\sc.exe'
$outputRoot = $null
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) {
    $config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
    $outputRoot = [string]$config.output_root
}

$cleanupAttempt = '{0}-{1}' -f ([DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')), ([Guid]::NewGuid().ToString('N'))
$cleanupEvidencePath = if ($outputRoot) {
    Join-Path $outputRoot "SYSTEM-CLEANUP-RESULT-$cleanupAttempt.json"
} else {
    $null
}
$scStopExitCode = $null
$serviceStateAfterStopWait = 'NOT_REQUESTED'
$servicePidAfterStopWait = 0L
$stopControlResult = 'NOT_REQUESTED'
$serviceRegistrationRemoved = $false
$serviceProcessGone = $false
$cliProcessGone = $false
$brokerProcesses = @()
$cliSessions = @()

function Get-SystemServiceSnapshot {
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$ServiceName'" -ErrorAction Stop |
        Select-Object -First 1
    if ($null -eq $service) {
        return [pscustomobject]@{
            present = $false
            state = 'ABSENT'
            process_id = 0L
            start_name = $null
        }
    }
    return [pscustomobject]@{
        present = $true
        state = [string]$service.State
        process_id = [int64]$service.ProcessId
        start_name = [string]$service.StartName
    }
}

function Get-OwnedSystemQualificationProcesses {
    $expectedPath = [IO.Path]::GetFullPath($ArtifactPath)
    @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue | Where-Object {
            try {
                $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expectedPath)
            }
            catch {
                $false
            }
        })
}

function Test-ExactProcessGone {
    param(
        [Parameter(Mandatory = $true)][int]$ProcessId,
        [Parameter(Mandatory = $true)][int64]$ProcessStartTime
    )
    try {
        $process = Get-Process -Id $ProcessId -ErrorAction Stop
        if ($ProcessStartTime -gt 0) {
            try {
                return ([int64]$process.StartTime.ToFileTimeUtc() -ne $ProcessStartTime)
            }
            catch {
                return $false
            }
        }
        return $false
    }
    catch {
        return $true
    }
}

function Write-SystemCleanupEvidence {
    if (-not $cleanupEvidencePath -or -not $outputRoot -or
        -not (Test-Path -LiteralPath $outputRoot -PathType Container)) {
        return
    }
    $result = [ordered]@{
        schema = 'amd-system-counter-cleanup/v1'
        qualification_only = $true
        service_name = $ServiceName
        cleanup_attempt = $cleanupAttempt
        cleanup_evidence_path = $cleanupEvidencePath
        sc_stop_exit_code = $scStopExitCode
        service_state_after_stop_wait = $serviceStateAfterStopWait
        service_pid_after_stop_wait = $servicePidAfterStopWait
        stop_control_result = $stopControlResult
        service_registration_removed = $serviceRegistrationRemoved
        service_process_gone_after_cleanup = $serviceProcessGone
        cli_process_gone_after_cleanup = $cliProcessGone
        broker_process_count_after_cleanup = @($brokerProcesses).Count
        cli_session_processes_checked = $cliSessions.Count
        amd_installation_mutated = $false
        amd_registry_mutated = $false
        note = 'Duplicate-safe SYSTEM cleanup evidence; each invocation uses a unique file and preserves prior attempts.'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    $result | ConvertTo-Json -Depth 10 |
        Set-Content -LiteralPath $cleanupEvidencePath -Encoding utf8
    Write-Host "SYSTEM cleanup evidence retained at $cleanupEvidencePath"
}

try {
    $initial = Get-SystemServiceSnapshot
    if ($initial.present) {
        & $sc stop $ServiceName | Out-Null
        $scStopExitCode = [int]$LASTEXITCODE

        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            $snapshot = Get-SystemServiceSnapshot
            if (-not $snapshot.present -or
                ($snapshot.state -eq 'Stopped' -and $snapshot.process_id -eq 0)) {
                break
            }
            Start-Sleep -Milliseconds 250
        } while ([DateTime]::UtcNow -lt $deadline)

        $snapshot = Get-SystemServiceSnapshot
        if ($snapshot.present) {
            $serviceStateAfterStopWait = $snapshot.state
            $servicePidAfterStopWait = [int64]$snapshot.process_id
        }
        else {
            $serviceStateAfterStopWait = 'ABSENT'
            $servicePidAfterStopWait = 0L
        }
        $stopControlResult = Resolve-QualificationStopDisposition `
            -StopExitCode $scStopExitCode `
            -ServiceState $serviceStateAfterStopWait `
            -ServiceProcessId $servicePidAfterStopWait `
            -ServicePresent $snapshot.present
        if ($stopControlResult -eq 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0') {
            Write-SystemCleanupEvidence
            throw "SYSTEM qualification service did not reach Stopped/PID 0 after sc.exe stop exit $scStopExitCode; cleanup failed closed."
        }

        if ($snapshot.present) {
            & $sc delete $ServiceName | Out-Null
            $deleteExitCode = [int]$LASTEXITCODE
            if ($deleteExitCode -ne 0) {
                Write-SystemCleanupEvidence
                throw "sc.exe delete failed with exit code $deleteExitCode"
            }
        }
        $serviceRegistrationRemoved = $true
    }
    else {
        $scStopExitCode = 1062
        $serviceStateAfterStopWait = 'ABSENT'
        $servicePidAfterStopWait = 0L
        $stopControlResult = 'SERVICE_ABSENT'
        $serviceRegistrationRemoved = $true
    }

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $snapshot = Get-SystemServiceSnapshot
        if (-not $snapshot.present) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    if ((Get-SystemServiceSnapshot).present) {
        Write-SystemCleanupEvidence
        throw "SYSTEM qualification service registration still exists: $ServiceName"
    }

    $brokerDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $brokerProcesses = @(Get-OwnedSystemQualificationProcesses)
        if ($brokerProcesses.Count -eq 0) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $brokerDeadline)
    $brokerProcesses = @(Get-OwnedSystemQualificationProcesses)
    $serviceProcessGone = ($brokerProcesses.Count -eq 0)

    if ($outputRoot -and (Test-Path -LiteralPath $outputRoot -PathType Container)) {
        foreach ($launchFile in @(Get-ChildItem -LiteralPath $outputRoot -Filter 'AMD-COUNTER-DISCOVERY-LAUNCH.json' -File -ErrorAction SilentlyContinue)) {
            $launch = Get-Content -LiteralPath $launchFile.FullName -Raw | ConvertFrom-Json
            $cliSessions += [pscustomobject]@{
                process_id = [int]$launch.target_pid
                process_start_time = [int64]$launch.target_process_start_time
            }
        }
    }
    $cliDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $cliProcessGone = $true
        foreach ($session in $cliSessions) {
            if (-not (Test-ExactProcessGone -ProcessId $session.process_id -ProcessStartTime $session.process_start_time)) {
                $cliProcessGone = $false
                break
            }
        }
        if ($cliProcessGone) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $cliDeadline)
    $cliProcessGone = $true
    foreach ($session in $cliSessions) {
        if (-not (Test-ExactProcessGone -ProcessId $session.process_id -ProcessStartTime $session.process_start_time)) {
            $cliProcessGone = $false
            break
        }
    }

    Write-SystemCleanupEvidence
    if (-not $serviceProcessGone -or -not $cliProcessGone) {
        throw 'Owned SYSTEM qualification or AMD CLI process identity remained after bounded cleanup wait; no unrelated process was killed.'
    }
    Write-Host "Removed only qualification service registration: $ServiceName"
}
catch {
    try { Write-SystemCleanupEvidence } catch { }
    throw
}
