#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ServiceName = 'ResourceTimelineAmdPrivilegeQualification'
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$QualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege'
$ConfigPath = Join-Path $QualificationRoot 'BROKER-CONFIG.json'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Cleanup requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
}
if (-not [Environment]::Is64BitProcess) {
    throw 'Cleanup requires x64 PowerShell.'
}

$sc = Join-Path $env:SystemRoot 'System32\sc.exe'
$service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($null -ne $service) {
    & $sc stop $ServiceName | Out-Null
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 1062) {
        throw "sc.exe stop failed with exit code $LASTEXITCODE"
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 250
        $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    } while ($null -ne $service -and $service.Status -ne 'Stopped' -and [DateTime]::UtcNow -lt $deadline)
    $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($null -ne $service -and $service.Status -ne 'Stopped') {
        throw 'Qualification service did not stop within the bounded cleanup wait.'
    }
    & $sc delete $ServiceName | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "sc.exe delete failed with exit code $LASTEXITCODE"
    }
}
$deadline = [DateTime]::UtcNow.AddSeconds(30)
while ($null -ne (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) -and [DateTime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 250
}
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    throw "Qualification service registration still exists: $ServiceName"
}

function Get-OwnedBrokerProcesses {
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

$brokerDeadline = [DateTime]::UtcNow.AddSeconds(30)
do {
    $brokerProcesses = @(Get-OwnedBrokerProcesses)
    if ($brokerProcesses.Count -eq 0) { break }
    Start-Sleep -Milliseconds 250
} while ([DateTime]::UtcNow -lt $brokerDeadline)
$serviceProcessGone = (@(Get-OwnedBrokerProcesses).Count -eq 0)

$outputRoot = $null
$cliSessions = @()
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) {
    $config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
    $outputRoot = [string]$config.output_root
    if ($outputRoot -and (Test-Path -LiteralPath $outputRoot -PathType Container)) {
        foreach ($launchFile in @(Get-ChildItem -LiteralPath $outputRoot -Filter 'AMD-CLI-LAUNCH-*.json' -File -ErrorAction SilentlyContinue)) {
            $launch = Get-Content -LiteralPath $launchFile.FullName -Raw | ConvertFrom-Json
            $cliSessions += [pscustomobject]@{
                process_id = [int]$launch.target_pid
                process_start_time = [int64]$launch.target_process_start_time
            }
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

$result = [ordered]@{
    schema = 'amd-privilege-cleanup/v1'
    qualification_only = $true
    service_name = $ServiceName
    service_registration_removed = $true
    service_process_gone_after_cleanup = $serviceProcessGone
    cli_process_gone_after_cleanup = $cliProcessGone
    broker_process_count_after_cleanup = @($brokerProcesses).Count
    cli_session_processes_checked = $cliSessions.Count
    amd_installation_mutated = $false
    amd_registry_mutated = $false
    note = 'Qualification evidence is retained for audit; only the owned service registration was removed.'
    recorded_at_utc = [DateTime]::UtcNow.ToString('o')
}
if ($outputRoot -and (Test-Path -LiteralPath $outputRoot -PathType Container)) {
        $result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $outputRoot 'CLEANUP-RESULT.json') -Encoding utf8
        Write-Host "Cleanup evidence retained at $outputRoot"
}
if (-not $serviceProcessGone -or -not $cliProcessGone) {
    throw 'Owned broker or AMD CLI process identity remained after bounded cleanup wait; evidence was retained and no unrelated process was killed.'
}
Write-Host "Removed only qualification service registration: $ServiceName"
