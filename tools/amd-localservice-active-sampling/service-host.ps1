[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ManifestPath,
    [switch]$Worker
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ToolRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $ToolRoot 'contract.ps1')
. (Join-Path $ToolRoot '..\amd-uprof-cli-spike\postprocess.ps1')
. (Join-Path $ToolRoot '..\amd-privilege-qualification\i2e-runtime-library.ps1')

function ConvertTo-JsonText {
    param([Parameter(Mandatory = $true)]$Value)
    $Value | ConvertTo-Json -Depth 50
}

function Write-JsonAtomic {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value,
        [switch]$AllowReplace
    )

    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    if ((Test-Path -LiteralPath $Path) -and -not $AllowReplace) {
        throw "refusing to replace immutable service evidence: $Path"
    }
    $temp = Join-Path $parent ('.pending-{0}.json' -f ([Guid]::NewGuid().ToString('N')))
    try {
        $encoding = New-Object System.Text.UTF8Encoding($false)
        [IO.File]::WriteAllText($temp, (ConvertTo-JsonText -Value $Value), $encoding)
        if ($AllowReplace) {
            Move-Item -LiteralPath $temp -Destination $Path -Force
        }
        else {
            Move-Item -LiteralPath $temp -Destination $Path
        }
    }
    finally {
        if (Test-Path -LiteralPath $temp) {
            Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
        }
    }
}

function Quote-WindowsArgument {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)

    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    $escaped = $Value -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    '"{0}"' -f $escaped
}

function Get-PeArchitecture {
    param([Parameter(Mandatory = $true)][string]$Path)

    try {
        $bytes = [IO.File]::ReadAllBytes($Path)
        if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
            return 'UNKNOWN'
        }
        $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
        if ($peOffset -lt 0 -or ($peOffset + 6) -gt $bytes.Length) {
            return 'UNKNOWN'
        }
        $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
        if ($machine -eq 0x8664) { return 'x64' }
        if ($machine -eq 0x014C) { return 'x86' }
        '0x{0:X4}' -f $machine
    }
    catch {
        'UNKNOWN'
    }
}

function Get-ManifestObject {
    if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
        throw 'manifest is missing'
    }
    Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
}

function Get-TokenPrivileges {
    $whoami = Join-Path $env:SystemRoot 'System32\whoami.exe'
    if (-not (Test-Path -LiteralPath $whoami -PathType Leaf)) {
        throw 'whoami.exe is missing; token privilege evidence is unavailable'
    }
    $lines = @(& $whoami /priv /fo csv /nh 2>&1)
    $records = New-Object System.Collections.Generic.List[object]
    foreach ($line in $lines) {
        $text = [string]$line
        if ($text -notmatch 'Se[A-Za-z]+Privilege') {
            continue
        }
        $csv = @($text | ConvertFrom-Csv -Header name, description, state)
        if ($csv.Count -eq 1) {
            [void]$records.Add([pscustomobject]@{
                name = ([string]$csv[0].name).Trim()
                state = ([string]$csv[0].state).Trim()
            })
        }
    }
    @($records)
}

function Get-EffectiveTokenEvidence {
    param([Parameter(Mandatory = $true)]$Manifest)

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $process = [Diagnostics.Process]::GetCurrentProcess()
    $groups = @()
    if ($null -ne $identity.Groups) {
        $groups = @($identity.Groups | ForEach-Object { $_.Value })
    }
    $integrity = @($groups | Where-Object { $_ -match '^S-1-16-' } | Select-Object -First 1)
    $parent = $null
    try {
        $parent = Get-CimInstance Win32_Process -Filter ("ProcessId={0}" -f $process.Id) |
            Select-Object -ExpandProperty ParentProcessId
    }
    catch {
        $parent = $null
    }
    [pscustomobject]@{
        user = $identity.Name
        user_sid = if ($null -ne $identity.User) { $identity.User.Value } else { $null }
        process_id = $process.Id
        parent_process_id = $parent
        session_id = $process.SessionId
        interactive = [Environment]::UserInteractive
        architecture = if ([IntPtr]::Size -eq 8) { 'x64' } else { 'x86' }
        integrity_sid = if ($integrity.Count -gt 0) { [string]$integrity[0] } else { $null }
        group_sids = $groups
        privileges = @(Get-TokenPrivileges)
        captured_at_utc = [DateTime]::UtcNow.ToString('o')
        service_name = $Manifest.service_name
    }
}

function Test-ManifestContract {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)]$Contract
    )

    $failures = New-Object System.Collections.Generic.List[string]
    $currentHarnessIdentity = Test-HarnessSourceIdentity -Root $ToolRoot -Contract $Contract
    if (-not $currentHarnessIdentity.valid) {
        [void]$failures.Add('reviewed harness source identity is not valid')
    }
    $manifestHarnessIdentity = Get-ContractPropertyValue -Object $Manifest -Name 'harness_identity'
    if ($null -eq $manifestHarnessIdentity -or -not [bool](Get-ContractPropertyValue -Object $manifestHarnessIdentity -Name 'valid' -Default $false)) {
        [void]$failures.Add('manifest harness source identity is missing or invalid')
    }
    elseif ($null -ne (Get-ContractPropertyValue -Object $manifestHarnessIdentity -Name 'source_files')) {
        foreach ($currentRecord in @($currentHarnessIdentity.source_files)) {
            $manifestRecord = @($manifestHarnessIdentity.source_files |
                Where-Object { [string]$_.key -ceq [string]$currentRecord.key } |
                Select-Object -First 1)
            if ($manifestRecord.Count -ne 1 -or
                [string]$manifestRecord[0].sha256 -cne [string]$currentRecord.sha256) {
                [void]$failures.Add(('manifest harness source identity mismatch: {0}' -f $currentRecord.key))
            }
        }
    }
    $expectedOutput = Join-Path ([string]$Manifest.run_root) 'raw\timechart-output'
    $expectedArgs = @(Get-FixedAmdCliArguments -OutputDirectory $expectedOutput)
    if ([string]$Manifest.schema -ne 'amd-localservice-active-sampling-q1/manifest/v1') {
        [void]$failures.Add('manifest schema mismatch')
    }
    if ([string]$Manifest.task_id -cne $Contract.task_id) {
        [void]$failures.Add('manifest task mismatch')
    }
    if ([string]$Manifest.service_name -cne $Contract.service_name) {
        [void]$failures.Add('manifest service name mismatch')
    }
    if ($null -eq $Manifest.gate) {
        [void]$failures.Add('manifest one-shot gate is missing')
    }
    else {
        if ([string]$Manifest.gate.task_id -cne $Contract.task_id) {
            [void]$failures.Add('manifest gate task mismatch')
        }
        if ([string]$Manifest.gate.state -cne 'CONSUMED') {
            [void]$failures.Add('manifest gate is not consumed')
        }
        if ([int]$Manifest.gate.max_runs -ne 1 -or [int]$Manifest.gate.retries -ne 0) {
            [void]$failures.Add('manifest gate budget is not one-shot with zero retries')
        }
        if (-not [bool]$Manifest.gate.consumed_before_service_registration) {
            [void]$failures.Add('manifest gate was not consumed before service registration')
        }
        if (-not [bool]$Manifest.gate.real_execution_allowed) {
            [void]$failures.Add('manifest gate does not allow the authorized live path')
        }
    }
    if ([string]$Manifest.service_account_sid -cne $Contract.account_sid) {
        [void]$failures.Add('manifest service account SID mismatch')
    }
    if ([string]$Manifest.service_sid_type -ine $Contract.service_sid_type) {
        [void]$failures.Add('manifest Service SID type mismatch')
    }
    if ([string]$Manifest.expected_service_sid -notmatch '^S-1-5-80-(?:\d+-){4}\d+$') {
        [void]$failures.Add('manifest expected Service SID is missing or malformed')
    }
    if ($null -eq $Manifest.service_sid_evidence -or
        [string]$Manifest.service_sid_evidence.sid -ine [string]$Manifest.expected_service_sid) {
        [void]$failures.Add('manifest Service SID evidence does not match the configured Service SID')
    }
    if ([int]$Manifest.session_id -ne 0 -or [bool]$Manifest.interactive) {
        [void]$failures.Add('manifest session/interactivity contract mismatch')
    }
    if ([string]$Manifest.executable -cne $Contract.amd_cli_path) {
        [void]$failures.Add('manifest AMD CLI path mismatch')
    }
    if ([string]$Manifest.output_directory -cne $expectedOutput) {
        [void]$failures.Add('manifest output directory mismatch')
    }
    if ([string]$Manifest.working_directory -cne (Split-Path -Parent $Contract.amd_cli_path)) {
        [void]$failures.Add('manifest working directory mismatch')
    }
    if ([int]$Manifest.duration_seconds -ne $Contract.amd_cli_duration_seconds -or
        [int]$Manifest.interval_ms -ne $Contract.amd_cli_interval_ms) {
        [void]$failures.Add('manifest duration or interval mismatch')
    }
    if ([string]$Manifest.expected_sha256 -cne $Contract.amd_cli_sha256) {
        [void]$failures.Add('manifest AMD CLI SHA256 mismatch')
    }
    if ([int]$Manifest.max_runs -ne 1 -or [int]$Manifest.retries -ne 0) {
        [void]$failures.Add('manifest run budget is not one-shot with zero retries')
    }
    $actualArgs = @($Manifest.arguments | ForEach-Object { [string]$_ })
    if (($actualArgs -join [char]0) -cne ($expectedArgs -join [char]0)) {
        [void]$failures.Add('manifest AMD CLI arguments mismatch')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        harness_identity = $currentHarnessIdentity
    }
}

function Get-ServiceBinaryIdentity {
    param([Parameter(Mandatory = $true)]$Manifest)

    $identity = [ordered]@{
        path = $Manifest.executable
        exists = $false
        sha256 = $null
        file_version = $null
        product_version = $null
        architecture = $null
        signature_status = $null
        signer = $null
        error = $null
    }
    try {
        if (-not (Test-Path -LiteralPath $Manifest.executable -PathType Leaf)) {
            $identity.error = 'AMD CLI file does not exist'
            return [pscustomobject]$identity
        }
        $identity.exists = $true
        $identity.sha256 = (Get-FileHash -LiteralPath $Manifest.executable -Algorithm SHA256).Hash.ToUpperInvariant()
        $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($Manifest.executable)
        $identity.file_version = $version.FileVersion
        $identity.product_version = $version.ProductVersion
        $identity.architecture = Get-PeArchitecture -Path $Manifest.executable
        $signature = Get-AuthenticodeSignature -FilePath $Manifest.executable
        $identity.signature_status = [string]$signature.Status
        if ($null -ne $signature.SignerCertificate) {
            $identity.signer = $signature.SignerCertificate.Subject
        }
    }
    catch {
        $identity.error = $_.Exception.Message
    }
    [pscustomobject]$identity
}

function Get-ProcessResultNotLaunched {
    param([string]$ErrorMessage = $null)

    [ordered]@{
        state = 'NOT_ATTEMPTED'
        executable = $null
        arguments = @()
        working_directory = $null
        output_directory = $null
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
        launch_intent_durable = $false
        launch_started_durable = $false
        launch_failed_durable = $false
        target_pid = $null
        started_at_utc = $null
        finished_at_utc = [DateTime]::UtcNow.ToString('o')
        duration_ms = 0
        timeout_ms = 30000
        timeout = $false
        target_exit_signed = $null
        target_exit_hex = $null
        stdout_path = $null
        stderr_path = $null
        stdout_bytes = 0
        stderr_bytes = 0
        stdout_persisted = $false
        stderr_persisted = $false
        capture_complete = $false
        child_tree_kill_attempted = $false
        cleanup_attempted = $false
        cleanup_succeeded = $false
        harness_failed = $true
        harness_error = $ErrorMessage
    }
}

function Try-WriteLaunchStartFailedEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$OutputDirectory,
        [Parameter(Mandatory = $true)][string]$ErrorMessage
    )

    try {
        Write-JsonAtomic -Path $Path -Value ([ordered]@{
            schema = 'amd-localservice-active-sampling-q1/cli-launch-start-failed/v1'
            state = 'PROCESS_START_FAILED'
            process_started = $false
            invocation_attempted = 0
            power_sampling_runs = 0
            launch_attempt_permitted = $true
            gate_consumed = $true
            start_result = 'FAILED'
            executable = $Manifest.executable
            arguments = @($Manifest.arguments)
            working_directory = $Manifest.working_directory
            output_directory = $OutputDirectory
            error = $ErrorMessage
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        })
        $true
    }
    catch {
        $false
    }
}

function Get-ProcessResultAfterStartFailure {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [AllowNull()][int]$TargetPid,
        [AllowNull()][DateTime]$StartedAt,
        [Parameter(Mandatory = $true)][string]$ErrorMessage,
        [Parameter(Mandatory = $true)][bool]$Timeout,
        [Parameter(Mandatory = $true)][bool]$KillAttempted,
        [AllowNull()][string]$KillOutput,
        [Parameter(Mandatory = $true)][bool]$ProcessTerminated
    )

    $rawRoot = Join-Path $RunRoot 'raw'
    $stdoutPath = Join-Path $rawRoot 'cli-stdout.txt'
    $stderrPath = Join-Path $rawRoot 'cli-stderr.txt'
    [ordered]@{
        state = 'PROCESS_FAILED_AFTER_START'
        executable = $Manifest.executable
        arguments = @($Manifest.arguments)
        working_directory = $Manifest.working_directory
        output_directory = (Join-Path $rawRoot 'timechart-output')
        process_started = $true
        invocation_attempted = 1
        power_sampling_runs = 1
        launch_intent_durable = (Test-Path -LiteralPath (Join-Path $rawRoot 'cli-launch-intent.json') -PathType Leaf)
        launch_started_durable = (Test-Path -LiteralPath (Join-Path $rawRoot 'cli-launch-started.json') -PathType Leaf)
        launch_failed_durable = (Test-Path -LiteralPath (Join-Path $rawRoot 'cli-launch-start-failed.json') -PathType Leaf)
        invocation_certainty = 'CONFIRMED_ONE'
        gate_consumed = $true
        target_pid = $TargetPid
        started_at_utc = if ($null -ne $StartedAt) { $StartedAt.ToString('o') } else { $null }
        finished_at_utc = [DateTime]::UtcNow.ToString('o')
        duration_ms = if ($null -ne $StartedAt) { [int64]([DateTime]::UtcNow - $StartedAt).TotalMilliseconds } else { $null }
        timeout_ms = [int]$Manifest.cli_timeout_ms
        timeout = $Timeout
        target_exit_signed = $null
        target_exit_hex = $null
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
        stdout_bytes = if (Test-Path -LiteralPath $stdoutPath -PathType Leaf) { ([IO.FileInfo]$stdoutPath).Length } else { 0 }
        stderr_bytes = if (Test-Path -LiteralPath $stderrPath -PathType Leaf) { ([IO.FileInfo]$stderrPath).Length } else { 0 }
        stdout_persisted = (Test-Path -LiteralPath $stdoutPath -PathType Leaf)
        stderr_persisted = (Test-Path -LiteralPath $stderrPath -PathType Leaf)
        capture_complete = (Test-Path -LiteralPath $stdoutPath -PathType Leaf) -and
            (Test-Path -LiteralPath $stderrPath -PathType Leaf)
        child_tree_kill_attempted = $KillAttempted
        child_tree_kill_output = $KillOutput
        cleanup_attempted = $true
        cleanup_succeeded = $ProcessTerminated
        harness_failed = $true
        harness_failure_after_process_start = $true
        harness_error = $ErrorMessage
    }
}

function Invoke-BoundedAmdCli {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $rawRoot = Join-Path $RunRoot 'raw'
    $outputDirectory = Join-Path $rawRoot 'timechart-output'
    $stdoutPath = Join-Path $rawRoot 'cli-stdout.txt'
    $stderrPath = Join-Path $rawRoot 'cli-stderr.txt'
    $launchPath = Join-Path $rawRoot 'cli-launch.json'
    $launchIntentPath = Join-Path $rawRoot 'cli-launch-intent.json'
    $launchStartedPath = Join-Path $rawRoot 'cli-launch-started.json'
    $launchFailedPath = Join-Path $rawRoot 'cli-launch-start-failed.json'
    $process = $null
    $stdoutTask = $null
    $stderrTask = $null
    $processStarted = $false
    $targetPid = $null
    $startedAt = $null
    $timeout = $false
    $killAttempted = $false
    $killOutput = $null
    try {
        $argumentText = @($Manifest.arguments | ForEach-Object {
            Quote-WindowsArgument -Value ([string]$_)
        }) -join ' '
        Write-JsonAtomic -Path $launchIntentPath -Value ([ordered]@{
            schema = 'amd-localservice-active-sampling-q1/cli-launch-intent/v1'
            state = 'LAUNCH_INTENT_DURABLE'
            launch_attempt_permitted = $true
            gate_consumed = $true
            start_result = 'UNKNOWN'
            executable = $Manifest.executable
            arguments = @($Manifest.arguments)
            working_directory = $Manifest.working_directory
            output_directory = $outputDirectory
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        })
        $psi = New-Object Diagnostics.ProcessStartInfo
        $psi.FileName = [string]$Manifest.executable
        $psi.Arguments = $argumentText
        $psi.WorkingDirectory = [string]$Manifest.working_directory
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $process = New-Object Diagnostics.Process
        $process.StartInfo = $psi
        if (-not $process.Start()) {
            $launchFailedDurable = Try-WriteLaunchStartFailedEvidence -Path $launchFailedPath -Manifest $Manifest -OutputDirectory $outputDirectory -ErrorMessage 'AMDuProfCLI process start returned false'
            $notStarted = Get-ProcessResultNotLaunched -ErrorMessage 'AMDuProfCLI process start returned false'
            $notStarted.state = 'LAUNCH_FAILED'
            $notStarted.executable = $Manifest.executable
            $notStarted.arguments = @($Manifest.arguments)
            $notStarted.working_directory = $Manifest.working_directory
            $notStarted.output_directory = $outputDirectory
            $notStarted.timeout_ms = [int]$Manifest.cli_timeout_ms
            $notStarted.launch_intent_durable = (Test-Path -LiteralPath $launchIntentPath -PathType Leaf)
            $notStarted.launch_failed_durable = $launchFailedDurable
            return $notStarted
        }
        $processStarted = $true
        $startedAt = [DateTime]::UtcNow
        try { $targetPid = [int]$process.Id } catch { $targetPid = $null }
        Write-JsonAtomic -Path $launchStartedPath -Value ([ordered]@{
            schema = 'amd-localservice-active-sampling-q1/cli-launch-started/v1'
            state = 'PROCESS_STARTED'
            process_started = $true
            invocation_attempted = 1
            power_sampling_runs = 1
            launch_attempt_permitted = $true
            gate_consumed = $true
            start_result = 'STARTED'
            target_pid = $targetPid
            executable = $Manifest.executable
            arguments = @($Manifest.arguments)
            working_directory = $Manifest.working_directory
            output_directory = $outputDirectory
            started_at_utc = $startedAt.ToString('o')
        })
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $launch = [ordered]@{
            schema = 'amd-localservice-active-sampling-q1/cli-launch/v1'
            process_started = $true
            target_pid = $targetPid
            executable = $Manifest.executable
            arguments = @($Manifest.arguments)
            working_directory = $Manifest.working_directory
            output_directory = $outputDirectory
            started_at_utc = $startedAt.ToString('o')
        }
        Write-JsonAtomic -Path $launchPath -Value $launch
        if (-not $process.WaitForExit([int]$Manifest.cli_timeout_ms)) {
            $timeout = $true
            $killAttempted = $true
            $killOutput = (& taskkill.exe /PID $targetPid /T /F 2>&1 | Out-String).Trim()
            $process.WaitForExit(5000)
        }
        $stdout = if ($null -ne $stdoutTask) { $stdoutTask.Result } else { '' }
        $stderr = if ($null -ne $stderrTask) { $stderrTask.Result } else { '' }
        [IO.File]::WriteAllText($stdoutPath, $stdout, (New-Object Text.UTF8Encoding($false)))
        [IO.File]::WriteAllText($stderrPath, $stderr, (New-Object Text.UTF8Encoding($false)))
        $exitCode = $null
        if (-not $timeout -and $process.HasExited) {
            $exitCode = [int]$process.ExitCode
        }
        $finish = [DateTime]::UtcNow
        [ordered]@{
            state = if ($timeout) { 'PROCESS_TIMEOUT' } else { 'PROCESS_COMPLETED' }
            executable = $Manifest.executable
            arguments = @($Manifest.arguments)
            working_directory = $Manifest.working_directory
            output_directory = $outputDirectory
            process_started = $true
            invocation_attempted = 1
            power_sampling_runs = 1
            launch_intent_durable = (Test-Path -LiteralPath $launchIntentPath -PathType Leaf)
            launch_started_durable = (Test-Path -LiteralPath $launchStartedPath -PathType Leaf)
            launch_failed_durable = $false
            invocation_certainty = 'CONFIRMED_ONE'
            gate_consumed = $true
            target_pid = $targetPid
            started_at_utc = $startedAt.ToString('o')
            finished_at_utc = $finish.ToString('o')
            duration_ms = [int64]($finish - $startedAt).TotalMilliseconds
            timeout_ms = [int]$Manifest.cli_timeout_ms
            timeout = $timeout
            target_exit_signed = $exitCode
            target_exit_hex = if ($null -ne $exitCode) { '0x{0:X8}' -f ([uint32]$exitCode) } else { $null }
            stdout_path = $stdoutPath
            stderr_path = $stderrPath
            stdout_bytes = [IO.FileInfo]$stdoutPath | Select-Object -ExpandProperty Length
            stderr_bytes = [IO.FileInfo]$stderrPath | Select-Object -ExpandProperty Length
            stdout_persisted = (Test-Path -LiteralPath $stdoutPath -PathType Leaf)
            stderr_persisted = (Test-Path -LiteralPath $stderrPath -PathType Leaf)
            capture_complete = (Test-Path -LiteralPath $stdoutPath -PathType Leaf) -and
                (Test-Path -LiteralPath $stderrPath -PathType Leaf)
            child_tree_kill_attempted = $killAttempted
            child_tree_kill_output = $killOutput
            cleanup_attempted = $true
            cleanup_succeeded = $process.HasExited
            harness_failed = $false
            harness_failure_after_process_start = $false
            harness_error = $null
        }
    }
    catch {
        if ($processStarted) {
            $processTerminated = $false
            try { $processTerminated = [bool]$process.HasExited } catch { $processTerminated = $false }
            if (-not $processTerminated -and $null -ne $targetPid) {
                try {
                    $killAttempted = $true
                    $killOutput = (& taskkill.exe /PID $targetPid /T /F 2>&1 | Out-String).Trim()
                }
                catch {
                    $killOutput = $_.Exception.Message
                }
                try { $processTerminated = [bool]$process.HasExited } catch { $processTerminated = $false }
            }
            return Get-ProcessResultAfterStartFailure -Manifest $Manifest -RunRoot $RunRoot -TargetPid $targetPid -StartedAt $startedAt -ErrorMessage $_.Exception.Message -Timeout $timeout -KillAttempted $killAttempted -KillOutput $killOutput -ProcessTerminated $processTerminated
        }
        $failure = Get-ProcessResultNotLaunched -ErrorMessage $_.Exception.Message
        $failure.state = 'LAUNCH_FAILED'
        $failure.executable = $Manifest.executable
        $failure.arguments = @($Manifest.arguments)
        $failure.working_directory = $Manifest.working_directory
        $failure.output_directory = $outputDirectory
        $failure.timeout_ms = [int]$Manifest.cli_timeout_ms
        $failure.launch_intent_durable = (Test-Path -LiteralPath $launchIntentPath -PathType Leaf)
        $failure.launch_failed_durable = if ($failure.launch_intent_durable) {
            Try-WriteLaunchStartFailedEvidence -Path $launchFailedPath -Manifest $Manifest -OutputDirectory $outputDirectory -ErrorMessage $_.Exception.Message
        }
        else {
            $false
        }
        if ($null -ne $process) {
            try {
                if (-not $process.HasExited) {
                    $killAttempted = $true
                    $killOutput = (& taskkill.exe /PID $process.Id /T /F 2>&1 | Out-String).Trim()
                }
            }
            catch {
                $killOutput = $_.Exception.Message
            }
        }
        $failure.child_tree_kill_attempted = $killAttempted
        $failure.child_tree_kill_output = $killOutput
        return $failure
    }
    finally {
        if ($null -ne $process) {
            $process.Dispose()
        }
    }
}

function Invoke-ServiceWorker {
    param([Parameter(Mandatory = $true)]$Manifest)

    $contract = Get-AmdLocalServiceSamplingContract
    $runRoot = [string]$Manifest.run_root
    $rawRoot = Join-Path $runRoot 'raw'
    $tokenPath = Join-Path $rawRoot 'token-evidence.json'
    $tokenValidationPath = Join-Path $rawRoot 'token-validation.json'
    $resultPath = Join-Path $rawRoot 'service-result.json'
    $statePath = Join-Path $rawRoot 'service-state.json'
    $manifestGate = Test-ManifestContract -Manifest $Manifest -Contract $contract
    if (-not $manifestGate.valid) {
        $blocked = [ordered]@{
            schema = 'amd-localservice-active-sampling-q1/service-result/v1'
            result = 'BLOCKED_MANIFEST_CONTRACT'
            token_validation = [pscustomobject]@{ valid = $false; failures = @('manifest contract invalid') }
            manifest_validation = $manifestGate
            process_result = Get-ProcessResultNotLaunched -ErrorMessage ($manifestGate.failures -join '; ')
            power_sampling_runs = 0
        }
        Write-JsonAtomic -Path $resultPath -Value $blocked
        return
    }
    $token = $null
    $tokenGate = $null
    try {
        $token = Get-EffectiveTokenEvidence -Manifest $Manifest
        Write-JsonAtomic -Path $tokenPath -Value $token
        $tokenGate = Test-EffectiveTokenEvidence -Evidence $token -Contract $contract -ExpectedServiceSid ([string]$Manifest.expected_service_sid)
        Write-JsonAtomic -Path $tokenValidationPath -Value $tokenGate
    }
    catch {
        $tokenGate = [pscustomobject]@{ valid = $false; failures = @($_.Exception.Message) }
        Write-JsonAtomic -Path $tokenValidationPath -Value $tokenGate
    }
    if (-not $tokenGate.valid) {
        $blocked = [ordered]@{
            schema = 'amd-localservice-active-sampling-q1/service-result/v1'
            result = 'BLOCKED_TOKEN_CONTRACT'
            manifest_validation = $manifestGate
            token = $token
            token_validation = $tokenGate
            binary_identity = $null
            process_result = Get-ProcessResultNotLaunched -ErrorMessage ($tokenGate.failures -join '; ')
            output_inventory = @(Get-OutputInventory -Root $runRoot)
            power_sampling_runs = 0
        }
        Write-JsonAtomic -Path $statePath -Value ([ordered]@{ state = 'TOKEN_BLOCKED'; at_utc = [DateTime]::UtcNow.ToString('o') })
        Write-JsonAtomic -Path $resultPath -Value $blocked
        return
    }
    $binary = Get-ServiceBinaryIdentity -Manifest $Manifest
    Write-JsonAtomic -Path (Join-Path $rawRoot 'binary-identity.json') -Value $binary
    $binaryGate = Test-BinaryIdentityEvidence -Identity $binary -Contract $contract
    if (-not $binaryGate.valid) {
        $blocked = [ordered]@{
            schema = 'amd-localservice-active-sampling-q1/service-result/v1'
            result = 'BLOCKED_BINARY_IDENTITY'
            manifest_validation = $manifestGate
            token = $token
            token_validation = $tokenGate
            binary_identity = $binary
            binary_validation = $binaryGate
            process_result = Get-ProcessResultNotLaunched -ErrorMessage ($binaryGate.failures -join '; ')
            output_inventory = @(Get-OutputInventory -Root $runRoot)
            power_sampling_runs = 0
        }
        Write-JsonAtomic -Path $statePath -Value ([ordered]@{ state = 'BINARY_BLOCKED'; at_utc = [DateTime]::UtcNow.ToString('o') })
        Write-JsonAtomic -Path $resultPath -Value $blocked
        return
    }
    Write-JsonAtomic -Path $statePath -Value ([ordered]@{ state = 'SAMPLING_ATTEMPTED_ONCE'; at_utc = [DateTime]::UtcNow.ToString('o') })
    $processResult = Invoke-BoundedAmdCli -Manifest $Manifest -RunRoot $runRoot
    Write-JsonAtomic -Path (Join-Path $rawRoot 'process-result.json') -Value $processResult
    $invocationAccounting = Get-InvocationAccounting -ProcessResult $processResult -RunRoot $runRoot -Q1GateEvidence (Get-Q1DictionaryValue -Object $Manifest -Name 'gate')
    $inventory = @(Get-OutputInventory -Root $runRoot)
    $result = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/service-result/v1'
        result = if ($processResult.timeout) { 'TIMEOUT' } elseif ($processResult.harness_failed) { 'HARNESS_FAILED' } elseif ($processResult.target_exit_signed -eq 0) { 'TARGET_COMPLETED_NEEDS_CSV_VALIDATION' } else { 'TARGET_FAILED' }
        manifest_validation = $manifestGate
        token = $token
        token_validation = $tokenGate
        binary_identity = $binary
        binary_validation = $binaryGate
        process_result = $processResult
        invocation_accounting = $invocationAccounting
        output_inventory = $inventory
        power_sampling_runs = $invocationAccounting.power_sampling_runs
        retries = 0
        completed_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonAtomic -Path $statePath -Value ([ordered]@{ state = 'SAMPLING_COMPLETE'; at_utc = [DateTime]::UtcNow.ToString('o') })
    Write-JsonAtomic -Path $resultPath -Value $result
}

$manifest = Get-ManifestObject
if ($Worker) {
    Invoke-ServiceWorker -Manifest $manifest
    exit 0
}

$serviceSource = @'
using System;
using System.Diagnostics;
using System.IO;
using System.ServiceProcess;

public sealed class AmdLocalServiceQualificationWindowsService : ServiceBase
{
    private readonly string scriptPath;
    private readonly string manifestPath;
    private readonly string powershellPath;
    private readonly string runRoot;
    private Process worker;

    public AmdLocalServiceQualificationWindowsService(string serviceName, string scriptPath, string manifestPath, string powershellPath, string runRoot)
    {
        this.ServiceName = serviceName;
        this.scriptPath = scriptPath;
        this.manifestPath = manifestPath;
        this.powershellPath = powershellPath;
        this.runRoot = runRoot;
        this.CanStop = true;
        this.CanShutdown = false;
        this.AutoLog = false;
    }

    protected override void OnStart(string[] args)
    {
        var psi = new ProcessStartInfo();
        psi.FileName = powershellPath;
        psi.Arguments = "-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File " +
            Quote(scriptPath) + " -Worker -ManifestPath " + Quote(manifestPath);
        psi.WorkingDirectory = Path.GetDirectoryName(scriptPath);
        psi.UseShellExecute = false;
        psi.CreateNoWindow = true;
        worker = Process.Start(psi);
        File.WriteAllText(Path.Combine(runRoot, "SERVICE-HOST-STARTED.txt"),
            DateTime.UtcNow.ToString("o") + " worker_pid=" + worker.Id.ToString());
    }

    protected override void OnStop()
    {
        try
        {
            if (worker != null && !worker.HasExited)
            {
                using (var killer = Process.Start(new ProcessStartInfo
                {
                    FileName = Environment.ExpandEnvironmentVariables("%SystemRoot%\\System32\\taskkill.exe"),
                    Arguments = "/PID " + worker.Id.ToString() + " /T /F",
                    UseShellExecute = false,
                    CreateNoWindow = true
                }))
                {
                    killer.WaitForExit(5000);
                }
            }
        }
        finally
        {
            File.WriteAllText(Path.Combine(runRoot, "SERVICE-HOST-STOPPED.txt"),
                DateTime.UtcNow.ToString("o"));
        }
    }

    private static string Quote(string value)
    {
        if (value.IndexOfAny(new[] { ' ', '\t', '"' }) < 0) return value;
        return "\"" + value.Replace("\"", "\\\"") + "\"";
    }
}
'@

if ($null -eq ('AmdLocalServiceQualificationWindowsService' -as [type])) {
    Add-Type -TypeDefinition $serviceSource -ReferencedAssemblies @('System.ServiceProcess.dll')
}
$powershellPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$service = New-Object AmdLocalServiceQualificationWindowsService(
    [string]$manifest.service_name,
    $MyInvocation.MyCommand.Path,
    $ManifestPath,
    $powershellPath,
    [string]$manifest.run_root
)
[System.ServiceProcess.ServiceBase]::Run([System.ServiceProcess.ServiceBase[]]@($service))
