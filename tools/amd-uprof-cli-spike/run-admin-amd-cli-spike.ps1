[CmdletBinding()]
param(
    [string]$InstallRoot,
    [string]$OutputRoot = (Join-Path $env:TEMP ('resource-timeline-amd-cli-spike-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'))),
    [int]$DurationSeconds = 10,
    [int]$SampleIntervalMs = 1000,
    [int]$TimeoutMs = 30000,
    [string]$ExpectedCliSha256
)

# Manual Administrator qualification only. This wrapper never self-elevates,
# changes PATH/environment/current directory globally, writes to the AMD tree,
# or starts a vendor process in a synthetic mode. The only target invocation is
# one bounded package-power timechart session.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$cliName = 'AMDuProfCLI.exe'
$installRegistryPath = 'Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\AMD\AMDProfiler'
$installRegistryValue = 'InstallationPath'

. (Join-Path $PSScriptRoot 'postprocess.ps1')

function Get-UtcTimestamp {
    (Get-Date).ToUniversalTime().ToString('o')
}

function Convert-ExitCodeToHex {
    param(
        [Parameter(Mandatory = $true)][int]$ExitCode
    )

    $bytes = [BitConverter]::GetBytes([int]$ExitCode)
    $unsigned = [BitConverter]::ToUInt32($bytes, 0)
    '0x{0:X8}' -f $unsigned
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )

    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    Write-Utf8File -Path $Path -Text ($Value | ConvertTo-Json -Depth 20)
}

function Get-PeMachine {
    param(
        [Parameter(Mandatory = $true)][string]$Path
    )

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
        throw "not a DOS PE image: $Path"
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    if ($peOffset -lt 0 -or $peOffset + 6 -gt $bytes.Length) {
        throw "invalid PE header offset: $Path"
    }
    if ($bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset + 1] -ne 0x45 -or
        $bytes[$peOffset + 2] -ne 0 -or $bytes[$peOffset + 3] -ne 0) {
        throw "missing PE signature: $Path"
    }
    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    [pscustomobject]@{
        machine_hex = ('0x{0:X4}' -f $machine)
        architecture = switch ($machine) {
            0x8664 { 'x64' }
            0x014C { 'x86' }
            0xAA64 { 'ARM64' }
            default { 'UNKNOWN' }
        }
    }
}

function Get-ArtifactRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$ExpectedSha256
    )

    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    $pe = Get-PeMachine -Path $Path
    $version = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
    $subject = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
    $issuer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Issuer } else { $null }
    $signerMatches = ($subject -match '(?i)AMD|Advanced Micro Devices') -or
        ($issuer -match '(?i)AMD|Advanced Micro Devices')
    $shaMatch = if ([string]::IsNullOrWhiteSpace($ExpectedSha256)) {
        $null
    } else {
        $hash -eq $ExpectedSha256.ToUpperInvariant()
    }

    [pscustomobject]@{
        path = $Path
        size_bytes = [long]$item.Length
        sha256 = $hash
        expected_sha256 = if ([string]::IsNullOrWhiteSpace($ExpectedSha256)) { $null } else { $ExpectedSha256.ToUpperInvariant() }
        sha256_match = $shaMatch
        machine = $pe.machine_hex
        architecture = $pe.architecture
        architecture_required = 'x64'
        architecture_match = ($pe.architecture -eq 'x64')
        file_version = $version.FileVersion
        product_version = $version.ProductVersion
        signature_status = $signature.Status.ToString()
        signature_subject = $subject
        signature_issuer = $issuer
        signer_matches_amd = $signerMatches
        signature_required = $true
        signature_requirement_passed = ($signature.Status.ToString() -eq 'Valid') -and $signerMatches
    }
}

function Get-AdminProof {
    param(
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    $whoamiPath = Join-Path $env:SystemRoot 'System32\whoami.exe'
    $whoamiLines = @(& $whoamiPath /groups 2>&1 | ForEach-Object { [string]$_ })
    $whoamiExit = [int]$LASTEXITCODE
    $whoamiText = $whoamiLines -join [Environment]::NewLine
    $whoamiPathOut = Join-Path $EvidenceRoot 'ADMIN-00-whoami-groups.txt'
    Write-Utf8File -Path $whoamiPathOut -Text $whoamiText

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $integritySids = @(
        [regex]::Matches($whoamiText, '(?i)\bS-1-16-\d+\b') |
            ForEach-Object { $_.Value.ToUpperInvariant() } |
            Select-Object -Unique
    )
    $integrityLevels = @(
        $integritySids | ForEach-Object { [int64]($_ -replace '^S-1-16-', '') }
    )
    $acceptedElevatedSids = @(
        $integritySids | Where-Object {
            [int64]($_ -replace '^S-1-16-', '') -ge 12288
        }
    )
    $powershellPath = $null
    try {
        $current = Get-Process -Id $PID -ErrorAction Stop
        $powershellPath = $current.Path
        $current.Dispose()
    } catch {
        $powershellPath = $null
    }

    [pscustomobject]@{
        test_id = 'ADMIN-00'
        timestamp_utc = Get-UtcTimestamp
        username = $identity.Name
        powershell_path = $powershellPath
        powershell_x64 = [Environment]::Is64BitProcess
        current_directory = (Get-Location).Path
        whoami_executable = $whoamiPath
        whoami_groups_exit = $whoamiExit
        whoami_groups_output_path = $whoamiPathOut
        whoami_groups_output = $whoamiText
        administrator_membership = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        parsed_integrity_sids = $integritySids
        parsed_integrity_levels = $integrityLevels
        accepted_elevated_integrity_sids = $acceptedElevatedSids
        accepted_elevated_integrity_present = ($acceptedElevatedSids.Count -gt 0)
        self_elevation_performed = $false
    }
}

function Get-ExistingCliProcesses {
    param(
        [Parameter(Mandatory = $true)][string]$CliPath
    )

    try {
        $rows = @(
            Get-CimInstance -ClassName Win32_Process -Filter "Name = '$cliName'" -ErrorAction Stop |
                Where-Object { $_.ExecutablePath -and $_.ExecutablePath.Equals($CliPath, [StringComparison]::OrdinalIgnoreCase) } |
                ForEach-Object {
                    [pscustomobject]@{
                        process_id = [int]$_.ProcessId
                        parent_process_id = [int]$_.ParentProcessId
                        executable_path = $_.ExecutablePath
                        command_line = $_.CommandLine
                    }
                }
        )
        [pscustomobject]@{ query_succeeded = $true; processes = $rows; error = $null }
    } catch {
        [pscustomobject]@{ query_succeeded = $false; processes = @(); error = $_.Exception.Message }
    }
}

function Get-DirectChildren {
    param(
        [Parameter(Mandatory = $true)][int]$ParentPid
    )

    try {
        $children = @(
            Get-CimInstance -ClassName Win32_Process -Filter ("ParentProcessId = {0}" -f $ParentPid) -ErrorAction Stop |
                ForEach-Object {
                    $alive = $false
                    try {
                        $child = Get-Process -Id ([int]$_.ProcessId) -ErrorAction Stop
                        $alive = -not $child.HasExited
                        $child.Dispose()
                    } catch {
                        $alive = $false
                    }
                    [pscustomobject]@{
                        process_id = [int]$_.ProcessId
                        parent_process_id = [int]$_.ParentProcessId
                        name = $_.Name
                        executable_path = $_.ExecutablePath
                        command_line = $_.CommandLine
                        alive = $alive
                    }
                }
        )
        [pscustomobject]@{ query_succeeded = $true; children = $children; error = $null }
    } catch {
        [pscustomobject]@{ query_succeeded = $false; children = @(); error = $_.Exception.Message }
    }
}

function Add-ProcessArgument {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.ProcessStartInfo]$StartInfo,
        [Parameter(Mandatory = $true)][string]$Argument
    )

    if ($StartInfo.PSObject.Properties.Name -contains 'ArgumentList') {
        [void]$StartInfo.ArgumentList.Add($Argument)
        return
    }

    if (-not [string]::IsNullOrEmpty($StartInfo.Arguments)) {
        $StartInfo.Arguments += ' '
    }
    $StartInfo.Arguments += (Quote-WindowsProcessArgument -Argument $Argument)
}

function Quote-WindowsProcessArgument {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Argument
    )

    if ($Argument.Length -gt 0 -and $Argument -notmatch '[\s"]') {
        return $Argument
    }

    $builder = [System.Text.StringBuilder]::new()
    [void]$builder.Append('"')
    $backslashes = 0
    foreach ($character in $Argument.ToCharArray()) {
        if ($character -eq '\') {
            $backslashes++
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($backslashes * 2) + 1)))
            [void]$builder.Append('"')
            $backslashes = 0
            continue
        }
        if ($backslashes -gt 0) {
            [void]$builder.Append(('\' * $backslashes))
            $backslashes = 0
        }
        [void]$builder.Append($character)
    }
    if ($backslashes -gt 0) {
        [void]$builder.Append(('\' * ($backslashes * 2)))
    }
    [void]$builder.Append('"')
    $builder.ToString()
}

function Invoke-CapturedCli {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][int]$TimeoutMs,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    $stdoutPath = Join-Path $EvidenceRoot 'AMD-CLI.stdout.txt'
    $stderrPath = Join-Path $EvidenceRoot 'AMD-CLI.stderr.txt'
    $startedAt = Get-UtcTimestamp
    $process = [System.Diagnostics.Process]::new()
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        Add-ProcessArgument -StartInfo $startInfo -Argument $argument
    }
    $process.StartInfo = $startInfo

    $processStarted = $false
    $timedOut = $false
    $cleanupAttempted = $false
    $cleanupSucceeded = $false
    $cleanupError = $null
    $targetPid = $null
    $targetExitSigned = $null
    $stdout = ''
    $stderr = ''
    $stdoutTask = $null
    $stderrTask = $null
    $harnessError = $null
    $peakWorkingSetBytes = 0L
    $targetCpuTimeMs = $null
    $directChildrenDuringRun = @()
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

    try {
        $processStarted = $process.Start()
        if (-not $processStarted) {
            throw "Process.Start returned false for $FilePath"
        }
        $targetPid = $process.Id
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        while (-not $process.HasExited -and $stopwatch.ElapsedMilliseconds -lt $TimeoutMs) {
            try {
                $process.Refresh()
                if ($process.WorkingSet64 -gt $peakWorkingSetBytes) {
                    $peakWorkingSetBytes = [int64]$process.WorkingSet64
                }
                $targetCpuTimeMs = $process.TotalProcessorTime.TotalMilliseconds
            } catch {
                # The target may exit between HasExited and Refresh; final capture
                # below remains authoritative for exit/output.
            }
            $childSnapshot = Get-DirectChildren -ParentPid $targetPid
            if ($childSnapshot.query_succeeded) {
                $directChildrenDuringRun = @($childSnapshot.children)
            }
            Start-Sleep -Milliseconds 100
        }
        if (-not $process.HasExited) {
            $timedOut = $true
            $cleanupAttempted = $true
            try {
                $process.Kill($true)
                $cleanupSucceeded = $true
            } catch {
                $cleanupError = $_.Exception.Message
                try {
                    $process.Kill()
                    $cleanupSucceeded = $true
                } catch {
                    $cleanupError = ($cleanupError + '; fallback: ' + $_.Exception.Message)
                }
            }
        }
        $process.WaitForExit()
        if (-not $timedOut) {
            $targetExitSigned = [int]$process.ExitCode
        }
        try {
            $process.Refresh()
            if ($process.WorkingSet64 -gt $peakWorkingSetBytes) {
                $peakWorkingSetBytes = [int64]$process.WorkingSet64
            }
            $targetCpuTimeMs = $process.TotalProcessorTime.TotalMilliseconds
        } catch {
            # Exit evidence is still retained if process statistics are unavailable.
        }
    } catch {
        $harnessError = $_.Exception.Message
    } finally {
        if ($processStarted -and $null -eq $targetExitSigned -and -not $timedOut) {
            try {
                if ($process.HasExited) {
                    $targetExitSigned = [int]$process.ExitCode
                }
            } catch {
                $harnessError = if ($harnessError) { $harnessError } else { $_.Exception.Message }
            }
        }
        if ($null -ne $stdoutTask) {
            try { $stdout = $stdoutTask.GetAwaiter().GetResult() } catch {
                $harnessError = if ($harnessError) { $harnessError } else { $_.Exception.Message }
            }
        }
        if ($null -ne $stderrTask) {
            try { $stderr = $stderrTask.GetAwaiter().GetResult() } catch {
                $harnessError = if ($harnessError) { $harnessError } else { $_.Exception.Message }
            }
        }
        $process.Dispose()
    }

    $stdoutPersisted = $false
    $stderrPersisted = $false
    try { Write-Utf8File -Path $stdoutPath -Text $stdout; $stdoutPersisted = $true } catch {
        $harnessError = if ($harnessError) { $harnessError } else { $_.Exception.Message }
    }
    try { Write-Utf8File -Path $stderrPath -Text $stderr; $stderrPersisted = $true } catch {
        $harnessError = if ($harnessError) { $harnessError } else { $_.Exception.Message }
    }

    $finishedAt = Get-UtcTimestamp
    [pscustomobject]@{
        process_started = $processStarted
        target_pid = $targetPid
        executable = $FilePath
        arguments = $Arguments
        working_directory = $WorkingDirectory
        started_at_utc = $startedAt
        finished_at_utc = $finishedAt
        duration_ms = [math]::Round($stopwatch.Elapsed.TotalMilliseconds, 3)
        timeout_ms = $TimeoutMs
        timeout = $timedOut
        target_exit_signed = $targetExitSigned
        target_exit_hex = if ($null -ne $targetExitSigned) { Convert-ExitCodeToHex -ExitCode $targetExitSigned } else { $null }
        target_process_failed = ($null -ne $targetExitSigned -and $targetExitSigned -ne 0) -or $timedOut
        peak_working_set_bytes = $peakWorkingSetBytes
        target_cpu_time_ms = $targetCpuTimeMs
        direct_children_during_run = $directChildrenDuringRun
        cleanup_attempted = $cleanupAttempted
        cleanup_succeeded = $cleanupSucceeded
        cleanup_error = $cleanupError
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
        stdout_bytes = ([System.Text.Encoding]::UTF8.GetByteCount($stdout))
        stderr_bytes = ([System.Text.Encoding]::UTF8.GetByteCount($stderr))
        stdout_persisted = $stdoutPersisted
        stderr_persisted = $stderrPersisted
        capture_complete = $stdoutPersisted -and $stderrPersisted -and ($null -eq $harnessError)
        harness_error = $harnessError
    }
}

if ($DurationSeconds -le 0 -or $SampleIntervalMs -le 0 -or $TimeoutMs -le 0) {
    throw 'duration, sample interval, and timeout must be greater than zero'
}

if ([string]::IsNullOrWhiteSpace($InstallRoot)) {
    $install = Get-ItemProperty -LiteralPath $installRegistryPath -Name $installRegistryValue -ErrorAction Stop
    $InstallRoot = [string]$install.PSObject.Properties[$installRegistryValue].Value
}
if ([string]::IsNullOrWhiteSpace($InstallRoot) -or -not [System.IO.Path]::IsPathRooted($InstallRoot)) {
    throw 'AMD uProf installation root was not an absolute path'
}
$InstallRoot = [System.IO.Path]::GetFullPath($InstallRoot)
if (-not ($InstallRoot -match '^[A-Za-z]:\\$')) {
    $InstallRoot = $InstallRoot.TrimEnd('\')
}
$binDirectory = Join-Path $InstallRoot 'bin'
$cliPath = Join-Path $binDirectory $cliName
$outputRootFull = [System.IO.Path]::GetFullPath($OutputRoot)
$installPrefix = $InstallRoot + '\'
if ($outputRootFull.Equals($InstallRoot, [StringComparison]::OrdinalIgnoreCase) -or
    $outputRootFull.StartsWith($installPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'output root must not be inside the AMD installation tree'
}
if (Test-Path -LiteralPath $outputRootFull) {
    throw "output root already exists; choose a fresh evidence directory: $outputRootFull"
}
[void](New-Item -ItemType Directory -Path $outputRootFull -Force)

$adminProof = Get-AdminProof -EvidenceRoot $outputRootFull
Write-JsonFile -Path (Join-Path $outputRootFull 'ADMIN-00-elevation-proof.json') -Value $adminProof
if (($adminProof.whoami_groups_exit -ne 0) -or
    -not $adminProof.administrator_membership -or
    -not $adminProof.accepted_elevated_integrity_present -or
    -not $adminProof.powershell_x64 -or
    $adminProof.self_elevation_performed) {
    throw 'Administrator x64 proof failed; no AMD CLI was launched'
}

$artifact = Get-ArtifactRecord -Path $cliPath -ExpectedSha256 $ExpectedCliSha256
Write-JsonFile -Path (Join-Path $outputRootFull 'CLI-ARTIFACT-PREFLIGHT.json') -Value $artifact
if (-not $artifact.architecture_match -or -not $artifact.signature_requirement_passed -or
    ($null -ne $artifact.sha256_match -and -not $artifact.sha256_match)) {
    throw 'AMDuProfCLI artifact preflight failed; no profiling was launched'
}

$existing = Get-ExistingCliProcesses -CliPath $cliPath
Write-JsonFile -Path (Join-Path $outputRootFull 'PREEXISTING-CLI-PROCESSES.json') -Value $existing
if (-not $existing.query_succeeded) {
    throw 'pre-existing AMDuProfCLI query was inconclusive; no profiling was launched'
}
if (@($existing.processes).Count -gt 0) {
    throw 'the exact AMDuProfCLI target is already running; no second instance was launched'
}

$environment = [pscustomobject]@{
    captured_at_utc = Get-UtcTimestamp
    powershell_x64 = [Environment]::Is64BitProcess
    current_directory = (Get-Location).Path
    path = $env:Path
    temp = $env:TEMP
    tmp = $env:TMP
    amd_variables = @(Get-ChildItem Env: | Where-Object { $_.Name -match '(?i)AMD|AMDT|UPROF' } | ForEach-Object {
        [pscustomobject]@{ name = $_.Name; value = $_.Value }
    })
}
Write-JsonFile -Path (Join-Path $outputRootFull 'COMMON-PROCESS-CONTEXT.json') -Value $environment

$sessionDirectory = Join-Path $outputRootFull 'timechart-output'
[void](New-Item -ItemType Directory -Path $sessionDirectory -Force)
$arguments = @(
    'timechart', '--event', 'power', '--interval', [string]$SampleIntervalMs,
    '--duration', [string]$DurationSeconds, '--format', 'csv', '--output-dir', $sessionDirectory
)
$run = Invoke-CapturedCli -FilePath $cliPath -Arguments $arguments -WorkingDirectory $binDirectory -TimeoutMs $TimeoutMs -EvidenceRoot $outputRootFull

# Persist the raw process capture before interpreting vendor output.
Write-JsonFile -Path (Join-Path $outputRootFull 'AMD-CLI-PROCESS-RESULT.json') -Value $run
$postRuntime = Invoke-AmdCliPostRuntimePipeline -SessionDirectory $sessionDirectory -Run $run
$csvPath = $postRuntime.timechart_csv_path
$uprofPath = $postRuntime.session_uprof_path
$parsed = $postRuntime.parsed_package_power
$inventory = $postRuntime.output_artifacts
$qualification = $postRuntime.qualification
$childrenAfter = if ($null -ne $run.target_pid) {
    Start-Sleep -Milliseconds 500
    Get-DirectChildren -ParentPid ([int]$run.target_pid)
} else {
    [pscustomobject]@{ query_succeeded = $false; children = @(); error = 'target PID unavailable' }
}

$summary = [pscustomobject]@{
    schema = 'cpu-sensor-amd-cli-spike/v1'
    created_at_utc = Get-UtcTimestamp
    install_root = $InstallRoot
    cli_path = $cliPath
    cli_sha256 = $artifact.sha256
    cli_version = $artifact.file_version
    cli_signature_status = $artifact.signature_status
    cli_arguments = $arguments
    working_directory = $binDirectory
    duration_seconds = $DurationSeconds
    sample_interval_ms = $SampleIntervalMs
    timeout_ms = $TimeoutMs
    process_result = $run
    output_directory = $sessionDirectory
    timechart_csv = $csvPath
    session_uprof = $uprofPath
    output_artifacts = $inventory
    parsed_package_power = $parsed
    direct_children_after = $childrenAfter
    orphan_processes_observed = if ($childrenAfter.query_succeeded) { @($childrenAfter.children | Where-Object { $_.alive }) } else { @() }
    qualification = $qualification
    amd_runtime_executed = $true
    debugger_used = $false
    persistent_environment_mutation = $false
}
Write-JsonFile -Path (Join-Path $outputRootFull 'ADMIN-AMD-CLI-SPIKE-SUMMARY.json') -Value $summary

Write-Output "Evidence root: $outputRootFull"
Write-Output "Qualification: $qualification"
