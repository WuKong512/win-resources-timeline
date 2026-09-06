#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'cleanup-state-contract.ps1')
. (Join-Path $PSScriptRoot 'i2e-service-profile-contract.ps1')

$ServiceName = $I2eServiceName
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$QualificationRoot = Join-Path $env:ProgramData $I2eOutputSubdirectory
$PointerPath = Join-Path $QualificationRoot 'I2E-EXPERIMENT-CURRENT.json'

function Assert-I2eCleanupAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'I2E cleanup requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
    }
    if (-not [Environment]::Is64BitProcess) { throw 'I2E cleanup requires x64 PowerShell.' }
}

function Read-I2eCleanupJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Write-I2eCleanupJson {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
}

function Invoke-I2eCleanupSc {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    $output = @(& $sc @Arguments 2>&1 | ForEach-Object { [string]$_ })
    if ($LASTEXITCODE -ne 0) {
        throw ('sc.exe {0} failed with exit code {1}: {2}' -f
            ($Arguments -join ' '), $LASTEXITCODE, ($output -join ' '))
    }
    $output
}

function Get-I2eCleanupServiceSnapshot {
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$ServiceName'" -ErrorAction Stop |
        Select-Object -First 1
    if ($null -eq $service) {
        return [pscustomobject]@{ present = $false; state = 'ABSENT'; process_id = 0L }
    }
    [pscustomobject]@{
        present = $true
        state = [string]$service.State
        process_id = [int64]$service.ProcessId
    }
}

function Stop-I2eCleanupService {
    $initial = Get-I2eCleanupServiceSnapshot
    if (-not $initial.present) {
        return [pscustomobject]@{ stop_exit_code = 1062; state = 'ABSENT'; process_id = 0L; disposition = 'SERVICE_ABSENT' }
    }
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    & $sc stop $ServiceName | Out-Null
    $exitCode = [int]$LASTEXITCODE
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $current = Get-I2eCleanupServiceSnapshot
        if (-not $current.present -or ($current.state -eq 'Stopped' -and $current.process_id -eq 0)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $current = Get-I2eCleanupServiceSnapshot
    $state = if ($current.present) { $current.state } else { 'ABSENT' }
    $pid = if ($current.present) { [int64]$current.process_id } else { 0L }
    $disposition = Resolve-QualificationStopDisposition -StopExitCode $exitCode -ServiceState $state -ServiceProcessId $pid -ServicePresent $current.present
    if ($disposition -eq 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0') {
        throw ('I2E cleanup failed closed; service state={0}, pid={1}, sc.exe exit={2}' -f $state, $pid, $exitCode)
    }
    [pscustomobject]@{ stop_exit_code = $exitCode; state = $state; process_id = $pid; disposition = $disposition }
}

function Remove-I2eCleanupService {
    $current = Get-I2eCleanupServiceSnapshot
    if (-not $current.present) { return $true }
    if ($current.state -ne 'Stopped' -or $current.process_id -ne 0) {
        throw 'Refusing to delete a running I2E service.'
    }
    Invoke-I2eCleanupSc -Arguments @('delete', $ServiceName) | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if (-not (Get-I2eCleanupServiceSnapshot).present) { return $true }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw ('I2E service registration remains after delete: {0}' -f $ServiceName)
}

function Test-I2eExactProcessGone {
    param([Parameter(Mandatory = $true)][int]$ProcessId, [Parameter(Mandatory = $true)][int64]$ProcessStartTime)
    try {
        $process = Get-Process -Id $ProcessId -ErrorAction Stop
        if ($ProcessStartTime -gt 0) {
            try { return ([int64]$process.StartTime.ToFileTimeUtc() -ne $ProcessStartTime) } catch { return $false }
        }
        return $false
    }
    catch { return $true }
}

function Get-I2eOwnedBrokerProcesses {
    $expected = [IO.Path]::GetFullPath($ArtifactPath)
    @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expected) } catch { $false }
    })
}

$null = Assert-I2eCleanupAdministrator
if (-not (Test-Path -LiteralPath $PointerPath -PathType Leaf)) {
    throw ('I2E experiment pointer is absent; refusing cleanup without an owned experiment: {0}' -f $PointerPath)
}
$pointer = Read-I2eCleanupJson -Path $PointerPath
$experimentId = [string]$pointer.experiment_id
$serviceSid = [string]$pointer.service_sid
# A pointer may survive service creation before Service SID resolution.
if ($experimentId -notmatch '^[0-9a-fA-F]{32}$') {
    throw 'I2E pointer has an invalid experiment identity.'
}
$rightNeedsRollback = [bool]$pointer.right_added_by_experiment -and -not [bool]$pointer.rollback_verified
if ($rightNeedsRollback -and $serviceSid -notmatch '^S-1-5-80-') {
    throw 'I2E pointer requires exact-right rollback but has no valid Service SID identity.'
}
$experimentRoot = Join-Path $QualificationRoot $experimentId
$attempt = '{0}-{1}' -f [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'), [Guid]::NewGuid().ToString('N')
$evidencePath = Join-Path $experimentRoot ('I2E-CLEANUP-RESULT-{0}.json' -f $attempt)
$stopResult = $null
$rightRollback = $false
$serviceRemoved = $false
$brokerGone = $false
$cliGone = $false
$cliSessions = @()

try {
    $stopResult = Stop-I2eCleanupService
    if ($rightNeedsRollback) {
        Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
        $direct = Get-I2eDirectAccountRightsSnapshot -Label 'cleanup-after-rollback' -Sid $serviceSid
        $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
        if ($direct.status -ne 'READ' -or $assigned.status -ne 'READ' -or
            @($direct.direct_rights) -contains $I2eRequiredRight -or
            @($assigned.assigned_principals) -contains $serviceSid) {
            throw 'I2E cleanup could not verify removal of the exact experiment right.'
        }
        $rightRollback = $true
        Write-I2eCleanupJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value ([ordered]@{
            schema = 'amd-service-profile-security-mutation-rollback/v1'
            qualification_only = $true
            experiment_id = $experimentId
            service_name = $ServiceName
            service_sid = $serviceSid
            right = $I2eRequiredRight
            right_added_by_experiment = $true
            rollback_at_utc = [DateTime]::UtcNow.ToString('o')
            rollback_verified = $true
            direct_verification = $direct
            assignment_verification = $assigned
            cleanup_invocation = $attempt
        })
    }
    else {
        $rightRollback = $true
    }

    if ($rightRollback) {
        Remove-I2eCleanupService | Out-Null
        $serviceRemoved = -not (Get-I2eCleanupServiceSnapshot).present
    }
    $brokerDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $brokers = @(Get-I2eOwnedBrokerProcesses)
        if ($brokers.Count -eq 0) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $brokerDeadline)
    $brokerGone = (@(Get-I2eOwnedBrokerProcesses).Count -eq 0)

    $roots = @(
        [string]$pointer.control_output_root,
        [string]$pointer.treatment_output_root
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) }
    foreach ($root in $roots) {
        foreach ($launchPath in @(Get-ChildItem -LiteralPath $root -Filter 'AMD-COUNTER-DISCOVERY-LAUNCH.json' -File -ErrorAction SilentlyContinue)) {
            $launch = Read-I2eCleanupJson -Path $launchPath.FullName
            $cliSessions += [pscustomobject]@{
                process_id = [int]$launch.target_pid
                process_start_time = [int64]$launch.target_process_start_time
            }
        }
    }
    $cliDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $cliGone = $true
        foreach ($session in $cliSessions) {
            if (-not (Test-I2eExactProcessGone -ProcessId $session.process_id -ProcessStartTime $session.process_start_time)) {
                $cliGone = $false
                break
            }
        }
        if ($cliGone) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $cliDeadline)
    if (-not $cliGone) { throw 'An owned AMD counter-discovery process identity remained; no unrelated process was killed.' }

    $result = [ordered]@{
        schema = 'amd-service-profile-cleanup/v1'
        qualification_only = $true
        experiment_id = $experimentId
        cleanup_attempt = $attempt
        cleanup_evidence_path = $evidencePath
        service_name = $ServiceName
        service_sid = $serviceSid
        stop_control = $stopResult
        right_rollback_verified = $rightRollback
        service_registration_removed = $serviceRemoved
        broker_process_count_after_cleanup = @(Get-I2eOwnedBrokerProcesses).Count
        service_process_gone_after_cleanup = $brokerGone
        cli_session_processes_checked = $cliSessions.Count
        cli_process_gone_after_cleanup = $cliGone
        amd_installation_mutated = $false
        amd_registry_mutated = $false
        lsa_mutation_scope = 'exact Service SID + SeSystemProfilePrivilege only; no AllRights'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-I2eCleanupJson -Path $evidencePath -Value $result
    Write-Host ('I2E cleanup evidence retained at {0}' -f $evidencePath)
}
catch {
    $failure = [ordered]@{
        schema = 'amd-service-profile-cleanup/v1'
        qualification_only = $true
        experiment_id = $experimentId
        cleanup_attempt = $attempt
        cleanup_evidence_path = $evidencePath
        service_name = $ServiceName
        service_sid = $serviceSid
        stop_control = $stopResult
        right_rollback_verified = $rightRollback
        service_registration_removed = $serviceRemoved
        service_process_gone_after_cleanup = $brokerGone
        cli_session_processes_checked = $cliSessions.Count
        cli_process_gone_after_cleanup = $cliGone
        error = $_.Exception.Message
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    try { Write-I2eCleanupJson -Path $evidencePath -Value $failure } catch { }
    throw
}
