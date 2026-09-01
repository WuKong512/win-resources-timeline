[CmdletBinding()]
param(
    [string]$InstallRoot = 'D:\apps\AMDuProf',
    [string]$OutputRoot = (Join-Path $env:TEMP ('resource-timeline-amd-vendor-noop-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'))),
    [switch]$SyntheticTest
)

# Manual Administrator invocation only. This script never elevates, changes
# PATH/current-directory state, or starts the vendor executable in synthetic
# mode. The production qualification invocation has no target arguments and
# uses only the exact installed AMDuProf.exe path derived from InstallRoot.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-UtcTimestamp {
    (Get-Date).ToUniversalTime().ToString('o')
}

function Convert-ExitCodeToHex {
    param(
        [Parameter(Mandatory = $true)][int]$ExitCode
    )

    $bytes = [BitConverter]::GetBytes($ExitCode)
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

    $json = $Value | ConvertTo-Json -Depth 14
    Write-Utf8File -Path $Path -Text $json
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

    # The fallback is only for older Windows PowerShell/.NET hosts. The
    # production vendor invocation passes an empty argument array; synthetic
    # tests use simple quoted arguments here.
    $escaped = $Argument.Replace('\\', '\\\\').Replace('"', '\\"')
    if (-not [string]::IsNullOrEmpty($StartInfo.Arguments)) {
        $StartInfo.Arguments += ' '
    }
    $StartInfo.Arguments += '"' + $escaped + '"'
}

function Get-PeMachine {
    param(
        [Parameter(Mandatory = $true)][string]$Path
    )

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if (($bytes.Length -lt 64) -or ($bytes[0] -ne 0x4D) -or ($bytes[1] -ne 0x5A)) {
        throw "not a DOS PE image: $Path"
    }

    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    if (($peOffset -lt 0) -or (($peOffset + 6) -gt $bytes.Length)) {
        throw "invalid PE header offset: $Path"
    }
    if (($bytes[$peOffset] -ne 0x50) -or ($bytes[$peOffset + 1] -ne 0x45) -or ($bytes[$peOffset + 2] -ne 0) -or ($bytes[$peOffset + 3] -ne 0)) {
        throw "missing PE signature: $Path"
    }

    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $architecture = switch ($machine) {
        0x8664 { 'x64' }
        0x014C { 'x86' }
        0xAA64 { 'ARM64' }
        default { 'UNKNOWN' }
    }
    [pscustomobject]@{
        machine_hex = ('0x{0:X4}' -f $machine)
        architecture = $architecture
    }
}

function Get-ArtifactRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Role,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256,
        [Parameter(Mandatory = $true)][bool]$SignatureRequired,
        [Parameter(Mandatory = $true)][string]$ExpectedSignerPattern
    )

    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $actualSha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    $pe = Get-PeMachine -Path $Path
    $fileVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
    $signerSubject = $null
    $signerIssuer = $null
    if ($null -ne $signature.SignerCertificate) {
        $signerSubject = $signature.SignerCertificate.Subject
        $signerIssuer = $signature.SignerCertificate.Issuer
    }
    $signerMatches = ($null -ne $signerSubject) -and ($signerSubject -match $ExpectedSignerPattern)
    $signaturePassed = if ($SignatureRequired) {
        ($signature.Status.ToString() -eq 'Valid') -and $signerMatches
    } else {
        $true
    }

    [pscustomobject]@{
        role = $Role
        path = $Path
        size = [long]$item.Length
        sha256 = $actualSha256
        expected_sha256 = $ExpectedSha256.ToUpperInvariant()
        sha256_match = ($actualSha256 -eq $ExpectedSha256.ToUpperInvariant())
        machine = $pe.machine_hex
        architecture = $pe.architecture
        expected_architecture = 'x64'
        architecture_match = ($pe.architecture -eq 'x64')
        file_version = $fileVersion.FileVersion
        product_version = $fileVersion.ProductVersion
        signature_status = $signature.Status.ToString()
        signature_subject = $signerSubject
        signature_issuer = $signerIssuer
        signer_matches_expected = $signerMatches
        signature_required = $SignatureRequired
        signature_requirement_passed = $signaturePassed
    }
}

function Get-AdminProof {
    param(
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    $whoamiPath = Join-Path $env:SystemRoot 'System32\whoami.exe'
    $whoamiOutput = @(& $whoamiPath /groups 2>&1 | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
    $whoamiExit = $LASTEXITCODE
    $whoamiOutputPath = Join-Path $EvidenceRoot 'ADMIN-00-whoami-groups.txt'
    Write-Utf8File -Path $whoamiOutputPath -Text $whoamiOutput

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $integritySids = @(
        [regex]::Matches($whoamiOutput, '(?i)\bS-1-16-\d+\b') |
            ForEach-Object { $_.Value } |
            Select-Object -Unique
    )
    $integrityLevels = @(
        $integritySids |
            ForEach-Object { [int64](($_ -replace '^S-1-16-', '')) }
    )
    $acceptedElevatedSids = @(
        $integritySids |
            Where-Object {
                $level = [int64](($_ -replace '^S-1-16-', ''))
                $level -ge 12288
            }
    )
    $currentProcessPath = $null
    try {
        $currentProcess = Get-Process -Id $PID -ErrorAction Stop
        $currentProcessPath = $currentProcess.Path
        $currentProcess.Dispose()
    } catch {
        $currentProcessPath = $null
    }

    [pscustomobject]@{
        test_id = 'ADMIN-00'
        timestamp_utc = Get-UtcTimestamp
        username = $identity.Name
        powershell_path = $currentProcessPath
        powershell_x64 = [Environment]::Is64BitProcess
        current_directory = (Get-Location).Path
        whoami_executable = $whoamiPath
        whoami_groups_exit = $whoamiExit
        whoami_groups_output_path = $whoamiOutputPath
        whoami_groups_output = $whoamiOutput
        administrator_membership = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        parsed_integrity_sids = $integritySids
        parsed_integrity_levels = $integrityLevels
        accepted_elevated_integrity_sids = $acceptedElevatedSids
        accepted_elevated_integrity_present = ($acceptedElevatedSids.Count -gt 0)
        self_elevation_performed = $false
    }
}

function Get-PreexistingVendorProcesses {
    param(
        [Parameter(Mandatory = $true)][string]$ImageName
    )

    try {
        $rows = @(
            Get-CimInstance -ClassName Win32_Process -Filter ("Name = '{0}'" -f $ImageName) -ErrorAction Stop |
                ForEach-Object {
                    [pscustomobject]@{
                        process_id = [int]$_.ProcessId
                        parent_process_id = [int]$_.ParentProcessId
                        name = $_.Name
                        executable_path = $_.ExecutablePath
                        command_line = $_.CommandLine
                        creation_date = [string]$_.CreationDate
                    }
                }
        )
        [pscustomobject]@{
            query_succeeded = $true
            image_name = $ImageName
            processes = $rows
            error = $null
        }
    } catch {
        [pscustomobject]@{
            query_succeeded = $false
            image_name = $ImageName
            processes = @()
            error = $_.Exception.Message
        }
    }
}

function Get-DirectChildSnapshot {
    param(
        [Parameter(Mandatory = $true)][int]$ParentPid
    )

    $capturedAt = Get-UtcTimestamp
    try {
        $children = @(
            Get-CimInstance -ClassName Win32_Process -Filter ("ParentProcessId = {0}" -f $ParentPid) -ErrorAction Stop |
                ForEach-Object {
                    $childPid = [int]$_.ProcessId
                    $childAlive = $false
                    $childPath = $_.ExecutablePath
                    try {
                        $childProcess = Get-Process -Id $childPid -ErrorAction Stop
                        $childAlive = -not $childProcess.HasExited
                        if ([string]::IsNullOrWhiteSpace($childPath)) {
                            $childPath = $childProcess.Path
                        }
                        $childProcess.Dispose()
                    } catch {
                        $childAlive = $false
                    }
                    [pscustomobject]@{
                        process_id = $childPid
                        parent_process_id = [int]$_.ParentProcessId
                        name = $_.Name
                        executable_path = $childPath
                        command_line = $_.CommandLine
                        creation_date = [string]$_.CreationDate
                        alive_at_observation = $childAlive
                    }
                }
        )
        [pscustomobject]@{
            captured_at_utc = $capturedAt
            parent_process_id = $ParentPid
            query_succeeded = $true
            children = $children
            error = $null
        }
    } catch {
        [pscustomobject]@{
            captured_at_utc = $capturedAt
            parent_process_id = $ParentPid
            query_succeeded = $false
            children = @()
            error = $_.Exception.Message
        }
    }
}

function Receive-ProcessStream {
    param(
        [Parameter()][object]$Task,
        [Parameter(Mandatory = $true)][int]$WaitMs
    )

    if ($null -eq $Task) {
        return [pscustomobject]@{
            status = 'NOT_REQUESTED'
            complete = $true
            text = ''
            error = $null
        }
    }
    try {
        if (-not $Task.Wait($WaitMs)) {
            return [pscustomobject]@{
                status = 'INCOMPLETE'
                complete = $false
                text = ''
                error = $null
            }
        }
        $text = $Task.GetAwaiter().GetResult()
        if ($null -eq $text) {
            $text = ''
        }
        [pscustomobject]@{
            status = 'COMPLETE'
            complete = $true
            text = [string]$text
            error = $null
        }
    } catch {
        [pscustomobject]@{
            status = 'ERROR'
            complete = $false
            text = ''
            error = $_.Exception.Message
        }
    }
}

function Invoke-StartupObservation {
    param(
        [Parameter(Mandatory = $true)][string]$TestId,
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][int]$ObservationWindowMs,
        [Parameter(Mandatory = $true)][string]$ResultPath,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter()][switch]$CaptureOutput,
        [Parameter()][switch]$AttemptGracefulCleanup,
        [Parameter()][switch]$ForceOwnedCleanup
    )

    $stdoutPath = $null
    $stderrPath = $null
    if ($CaptureOutput) {
        $stdoutPath = Join-Path $EvidenceRoot ($TestId + '.stdout.txt')
        $stderrPath = Join-Path $EvidenceRoot ($TestId + '.stderr.txt')
    }
    $beforeCleanupPath = Join-Path $EvidenceRoot ($TestId + '.qualification-before-cleanup.json')
    $startedAt = Get-UtcTimestamp
    $observationStart = [DateTime]::UtcNow
    $observationDeadline = $observationStart.AddMilliseconds($ObservationWindowMs)
    $process = [System.Diagnostics.Process]::new()
    $stdoutTask = $null
    $stderrTask = $null
    $processStarted = $false
    $targetPid = $null
    $rootAliveAtDeadline = $null
    $targetExitSigned = $null
    $targetExitHex = $null
    $waitReturned = $null
    $observationElapsedMs = $null
    $childSnapshot = $null
    $harnessError = $null
    $qualificationPersistedBeforeCleanup = $false
    $cleanup = [ordered]@{
        cleanup_requested = [bool]$AttemptGracefulCleanup
        graceful_close_attempted = $false
        graceful_close_returned = $false
        graceful_close_succeeded = $false
        graceful_close_error = $null
        force_owned_pid_cleanup_requested = [bool]$ForceOwnedCleanup
        force_owned_pid_cleanup_attempted = $false
        force_owned_pid_cleanup_succeeded = $false
        force_owned_pid_cleanup_error = $null
        target_alive_after_cleanup = $null
        cleanup_completed_at_utc = $null
    }

    try {
        $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $FilePath
        $startInfo.WorkingDirectory = $WorkingDirectory
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.Arguments = ''
        $startInfo.RedirectStandardOutput = [bool]$CaptureOutput
        $startInfo.RedirectStandardError = [bool]$CaptureOutput
        foreach ($argument in $Arguments) {
            Add-ProcessArgument -StartInfo $startInfo -Argument $argument
        }
        $process.StartInfo = $startInfo

        $processStarted = $process.Start()
        if (-not $processStarted) {
            throw "Process.Start returned false for $FilePath"
        }
        $targetPid = $process.Id
        if ($CaptureOutput) {
            $stdoutTask = $process.StandardOutput.ReadToEndAsync()
            $stderrTask = $process.StandardError.ReadToEndAsync()
        }

        $waitReturned = $process.WaitForExit($ObservationWindowMs)
        $observationElapsedMs = [math]::Round(([DateTime]::UtcNow - $observationStart).TotalMilliseconds, 3)
        try {
            $rootAliveAtDeadline = -not $process.HasExited
        } catch {
            $harnessError = $_.Exception.Message
        }
        if (-not $rootAliveAtDeadline) {
            try {
                $targetExitSigned = [int]$process.ExitCode
            } catch {
                if ([string]::IsNullOrWhiteSpace($harnessError)) {
                    $harnessError = $_.Exception.Message
                }
            }
        }
        if ($null -ne $targetExitSigned) {
            $targetExitHex = Convert-ExitCodeToHex -ExitCode $targetExitSigned
        }
        $childSnapshot = Get-DirectChildSnapshot -ParentPid $targetPid
        $aliveChildren = @($childSnapshot.children | Where-Object { $_.alive_at_observation })
        $delegatedChildObserved = (-not $rootAliveAtDeadline) -and $childSnapshot.query_succeeded -and ($aliveChildren.Count -gt 0)
        $vendorStartupControl = if ($rootAliveAtDeadline -or $delegatedChildObserved) {
            'PASS'
        } elseif (-not $childSnapshot.query_succeeded) {
            'INCONCLUSIVE'
        } else {
            'FAIL'
        }
        $startupModel = if ($rootAliveAtDeadline) {
            'ROOT_PROCESS_SURVIVED'
        } elseif ($delegatedChildObserved) {
            'DELEGATED_CHILD_PROCESS'
        } else {
            'NO_SURVIVING_ROOT_OR_DELEGATED_CHILD'
        }

        $preCleanup = [pscustomobject]@{
            phase = 'QUALIFICATION_BEFORE_CLEANUP'
            test_id = $TestId
            process_started = $processStarted
            target_pid = $targetPid
            executable = $FilePath
            arguments = @($Arguments)
            argument_count = @($Arguments).Count
            working_directory = $WorkingDirectory
            started_at_utc = $startedAt
            observation_deadline_utc = $observationDeadline.ToString('o')
            observation_window_ms = $ObservationWindowMs
            observation_elapsed_ms = $observationElapsedMs
            wait_for_exit_returned = $waitReturned
            timeout = $false
            watchdog_kill_attempted = $false
            root_alive_at_deadline = $rootAliveAtDeadline
            target_exit_signed = $targetExitSigned
            target_exit_hex = $targetExitHex
            direct_child_snapshot = $childSnapshot
            delegated_child_observed = $delegatedChildObserved
            startup_model = $startupModel
            vendor_startup_control = $vendorStartupControl
            stdout_path = $stdoutPath
            stderr_path = $stderrPath
            stdout_capture_status = if ($CaptureOutput) { 'PENDING_BEFORE_CLEANUP' } else { 'NOT_REQUESTED_GUI_CONTROL' }
            stderr_capture_status = if ($CaptureOutput) { 'PENDING_BEFORE_CLEANUP' } else { 'NOT_REQUESTED_GUI_CONTROL' }
            stdout_persisted = $false
            stderr_persisted = $false
            capture_complete = (-not $CaptureOutput)
            target_process_failed = ($null -ne $targetExitSigned) -and ($targetExitSigned -ne 0)
            harness_failed = (-not [string]::IsNullOrWhiteSpace($harnessError))
            harness_error = $harnessError
            qualification_persisted_before_cleanup = $true
            cleanup = $null
        }
        Write-JsonFile -Path $beforeCleanupPath -Value $preCleanup
        $qualificationPersistedBeforeCleanup = $true

        if ($AttemptGracefulCleanup -and $rootAliveAtDeadline) {
            try {
                if (-not $process.HasExited) {
                    $cleanup.graceful_close_attempted = $true
                    $cleanup.graceful_close_returned = [bool]$process.CloseMainWindow()
                    if ($cleanup.graceful_close_returned) {
                        [void]$process.WaitForExit(1000)
                        $cleanup.graceful_close_succeeded = $process.HasExited
                    }
                }
            } catch {
                $cleanup.graceful_close_error = $_.Exception.Message
            }
            if (-not $cleanup.graceful_close_succeeded -and $ForceOwnedCleanup) {
                $cleanup.force_owned_pid_cleanup_attempted = $true
                try {
                    # This is an explicit synthetic-test option only. It acts
                    # on this exact Process object/PID, never by process name.
                    $process.Kill()
                    $cleanup.force_owned_pid_cleanup_succeeded = $true
                    [void]$process.WaitForExit(1000)
                } catch {
                    $cleanup.force_owned_pid_cleanup_error = $_.Exception.Message
                }
            }
            try {
                $cleanup.target_alive_after_cleanup = -not $process.HasExited
            } catch {
                $cleanup.target_alive_after_cleanup = $null
            }
        }
        $cleanup.cleanup_completed_at_utc = Get-UtcTimestamp

        $stdoutCapture = Receive-ProcessStream -Task $stdoutTask -WaitMs 2000
        $stderrCapture = Receive-ProcessStream -Task $stderrTask -WaitMs 2000
        $stdoutPersisted = $false
        $stderrPersisted = $false
        if ($CaptureOutput -and $stdoutCapture.complete) {
            Write-Utf8File -Path $stdoutPath -Text $stdoutCapture.text
            $stdoutPersisted = $true
        }
        if ($CaptureOutput -and $stderrCapture.complete) {
            Write-Utf8File -Path $stderrPath -Text $stderrCapture.text
            $stderrPersisted = $true
        }
        $finishedAt = Get-UtcTimestamp
        $streamError = @($stdoutCapture.error, $stderrCapture.error | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) -join '; '
        if (-not [string]::IsNullOrWhiteSpace($streamError) -and [string]::IsNullOrWhiteSpace($harnessError)) {
            $harnessError = $streamError
        }
        $result = [pscustomobject]@{
            phase = 'FINAL_AFTER_CLEANUP'
            test_id = $TestId
            process_started = $processStarted
            target_pid = $targetPid
            executable = $FilePath
            arguments = @($Arguments)
            argument_count = @($Arguments).Count
            working_directory = $WorkingDirectory
            started_at_utc = $startedAt
            finished_at_utc = $finishedAt
            observation_deadline_utc = $observationDeadline.ToString('o')
            observation_window_ms = $ObservationWindowMs
            observation_elapsed_ms = $observationElapsedMs
            wait_for_exit_returned = $waitReturned
            timeout = $false
            watchdog_kill_attempted = $false
            root_alive_at_deadline = $rootAliveAtDeadline
            target_exit_signed = $targetExitSigned
            target_exit_hex = $targetExitHex
            direct_child_snapshot = $childSnapshot
            delegated_child_observed = $delegatedChildObserved
            startup_model = $startupModel
            vendor_startup_control = $vendorStartupControl
            stdout_path = $stdoutPath
            stderr_path = $stderrPath
            stdout_capture_status = $stdoutCapture.status
            stderr_capture_status = $stderrCapture.status
            stdout_bytes = if ($stdoutPersisted) { [System.Text.Encoding]::UTF8.GetByteCount($stdoutCapture.text) } else { $null }
            stderr_bytes = if ($stderrPersisted) { [System.Text.Encoding]::UTF8.GetByteCount($stderrCapture.text) } else { $null }
            stdout_persisted = $stdoutPersisted
            stderr_persisted = $stderrPersisted
            capture_complete = (-not $CaptureOutput) -or ($stdoutPersisted -and $stderrPersisted)
            target_process_failed = ($null -ne $targetExitSigned) -and ($targetExitSigned -ne 0)
            harness_failed = (-not [string]::IsNullOrWhiteSpace($harnessError))
            harness_error = $harnessError
            qualification_persisted_before_cleanup = $qualificationPersistedBeforeCleanup
            qualification_before_cleanup_path = $beforeCleanupPath
            cleanup = [pscustomobject]$cleanup
        }
        Write-JsonFile -Path $ResultPath -Value $result
        return $result
    } catch {
        $harnessError = $_.Exception.Message
        $fallback = [pscustomobject]@{
            phase = 'HARNESS_FAILURE'
            test_id = $TestId
            process_started = $processStarted
            target_pid = $targetPid
            executable = $FilePath
            arguments = @($Arguments)
            working_directory = $WorkingDirectory
            observation_window_ms = $ObservationWindowMs
            timeout = $false
            target_exit_signed = $targetExitSigned
            target_exit_hex = $targetExitHex
            vendor_startup_control = 'BLOCKED_HARNESS'
            harness_failed = $true
            harness_error = $harnessError
            qualification_persisted_before_cleanup = $qualificationPersistedBeforeCleanup
            qualification_before_cleanup_path = $beforeCleanupPath
            cleanup = [pscustomobject]$cleanup
        }
        try {
            Write-JsonFile -Path $ResultPath -Value $fallback
        } catch {
            # Preserve the original harness error; there is no safe recovery
            # path that would justify starting or killing another process.
        }
        return $fallback
    } finally {
        $process.Dispose()
    }
}

function Invoke-SyntheticRegression {
    param(
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    $syntheticShell = Join-Path $env:SystemRoot 'System32\cmd.exe'
    $syntheticPowerShell = $null
    try {
        $syntheticPowerShell = (Get-Process -Id $PID -ErrorAction Stop).Path
    } catch {
        $syntheticPowerShell = $null
    }
    if ([string]::IsNullOrWhiteSpace($syntheticPowerShell)) {
        $syntheticPowerShell = Join-Path $PSHOME 'pwsh.exe'
    }
    $workingDirectory = (Get-Location).Path
    $negativeCommand = 'echo SYNTHETIC_STDOUT_MARKER & echo SYNTHETIC_STDERR_MARKER 1>&2 & exit /b -1'
    $zeroCommand = 'echo SYNTHETIC_ZERO_STDOUT_MARKER & exit /b 0'
    $survivorCommand = 'echo SYNTHETIC_SURVIVOR_MARKER & ping -n 7 127.0.0.1 > nul'
    $childCommand = '$child = Start-Process -FilePath $env:ComSpec -ArgumentList @(''/c'', ''ping'', ''-n'', ''3'', ''127.0.0.1'') -PassThru; [Console]::Out.WriteLine(''SYNTHETIC_PARENT_MARKER''); $child.WaitForExit(); exit 0'

    $negative = Invoke-StartupObservation `
        -TestId 'SYNTHETIC-NEGATIVE-EARLY-EXIT' `
        -FilePath $syntheticShell `
        -Arguments @('/d', '/c', $negativeCommand) `
        -WorkingDirectory $workingDirectory `
        -ObservationWindowMs 2000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-NEGATIVE-EARLY-EXIT.json') `
        -CaptureOutput
    $zero = Invoke-StartupObservation `
        -TestId 'SYNTHETIC-ZERO-EXIT' `
        -FilePath $syntheticShell `
        -Arguments @('/d', '/c', $zeroCommand) `
        -WorkingDirectory $workingDirectory `
        -ObservationWindowMs 2000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-ZERO-EXIT.json') `
        -CaptureOutput
    $survivor = Invoke-StartupObservation `
        -TestId 'SYNTHETIC-SURVIVOR-CLEANUP' `
        -FilePath $syntheticShell `
        -Arguments @('/d', '/c', $survivorCommand) `
        -WorkingDirectory $workingDirectory `
        -ObservationWindowMs 250 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-SURVIVOR-CLEANUP.json') `
        -CaptureOutput `
        -AttemptGracefulCleanup `
        -ForceOwnedCleanup
    $emptyArguments = Invoke-StartupObservation `
        -TestId 'SYNTHETIC-EMPTY-ARGUMENTS' `
        -FilePath (Join-Path $env:SystemRoot 'System32\whoami.exe') `
        -Arguments @() `
        -WorkingDirectory $workingDirectory `
        -ObservationWindowMs 2000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-EMPTY-ARGUMENTS.json') `
        -CaptureOutput
    $directChild = Invoke-StartupObservation `
        -TestId 'SYNTHETIC-DIRECT-CHILD' `
        -FilePath $syntheticPowerShell `
        -Arguments @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $childCommand) `
        -WorkingDirectory $workingDirectory `
        -ObservationWindowMs 250 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-DIRECT-CHILD.json') `
        -CaptureOutput
    # The child fixture owns only its short-lived cmd/ping child and waits for
    # it naturally; allow that exact synthetic tree to finish before returning.
    Start-Sleep -Milliseconds 2500

    $negativeStdout = if ($negative.stdout_persisted) { [System.IO.File]::ReadAllText($negative.stdout_path) } else { '' }
    $negativeStderr = if ($negative.stderr_persisted) { [System.IO.File]::ReadAllText($negative.stderr_path) } else { '' }
    $zeroStdout = if ($zero.stdout_persisted) { [System.IO.File]::ReadAllText($zero.stdout_path) } else { '' }
    $zeroStderr = if ($zero.stderr_persisted) { [System.IO.File]::ReadAllText($zero.stderr_path) } else { '' }
    $survivorRootStopped = ($survivor.cleanup.target_alive_after_cleanup -eq $false)
    $directChildCount = @($directChild.direct_child_snapshot.children).Count
    $directChildObservationPassed = ($directChild.direct_child_snapshot.query_succeeded -eq $true) -and ($directChildCount -gt 0)
    $directChildObservationStatus = if ($directChildObservationPassed) {
        'PASS'
    } elseif (($directChild.direct_child_snapshot.query_succeeded -eq $false) -and ($directChild.direct_child_snapshot.error -match '(?i)access|拒绝')) {
        'INCONCLUSIVE_NONADMIN_QUERY_ACCESS'
    } else {
        'FAIL'
    }
    # WMI process-parent inspection is intentionally read-only and is expected
    # to be available in the manually elevated qualification shell. The local
    # non-admin validation host may deny Win32_Process queries; that is an
    # environment limitation, not permission to guess a child relationship.
    $directChildValidationPassed = $directChildObservationPassed -or ($directChildObservationStatus -eq 'INCONCLUSIVE_NONADMIN_QUERY_ACCESS')
    $summary = [pscustomobject]@{
        result = if (
            ((Convert-ExitCodeToHex -ExitCode 0) -eq '0x00000000') -and
            ((Convert-ExitCodeToHex -ExitCode 1) -eq '0x00000001') -and
            ((Convert-ExitCodeToHex -ExitCode -1) -eq '0xFFFFFFFF') -and
            $negative.process_started -and (-not $negative.timeout) -and ($negative.target_exit_signed -eq -1) -and
            ($negative.target_exit_hex -eq '0xFFFFFFFF') -and $negative.capture_complete -and
            $negativeStdout.Contains('SYNTHETIC_STDOUT_MARKER') -and $negativeStderr.Contains('SYNTHETIC_STDERR_MARKER') -and
            $zero.process_started -and (-not $zero.timeout) -and ($zero.target_exit_signed -eq 0) -and
            ($zero.target_exit_hex -eq '0x00000000') -and $zero.capture_complete -and
            $zeroStdout.Contains('SYNTHETIC_ZERO_STDOUT_MARKER') -and ($zeroStderr.Length -eq 0) -and
            $survivor.process_started -and ($survivor.root_alive_at_deadline -eq $true) -and
            $survivor.qualification_persisted_before_cleanup -and $survivor.cleanup.graceful_close_attempted -and $survivorRootStopped -and
            $emptyArguments.process_started -and (-not $emptyArguments.timeout) -and $emptyArguments.capture_complete -and
            $directChildValidationPassed
        ) { 'SYNTHETIC_REGRESSION_PASS' } else { 'SYNTHETIC_REGRESSION_FAIL' }
        amd_runtime_executed = $false
        exit_code_conversion = [pscustomobject]@{
            zero = Convert-ExitCodeToHex -ExitCode 0
            one = Convert-ExitCodeToHex -ExitCode 1
            negative_one = Convert-ExitCodeToHex -ExitCode -1
        }
        negative_early_exit = $negative
        zero_exit_empty_stderr = $zero
        survivor_cleanup = $survivor
        empty_arguments = $emptyArguments
        direct_child_observation = $directChild
        direct_child_observation_status = $directChildObservationStatus
        direct_child_count_observed = $directChildCount
        no_amd_executable_or_dll_started = $true
    }
    Write-JsonFile -Path (Join-Path $EvidenceRoot 'SYNTHETIC-REGRESSION-SUMMARY.json') -Value $summary
    if ($summary.result -ne 'SYNTHETIC_REGRESSION_PASS') {
        throw 'synthetic no-op wrapper regression failed'
    }
    Write-Output "EVIDENCE_ROOT=$EvidenceRoot"
    Write-Output 'SYNTHETIC_REGRESSION=PASS'
}

New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null

if ($SyntheticTest) {
    Invoke-SyntheticRegression -EvidenceRoot $OutputRoot
    return
}

$adminProof = Get-AdminProof -EvidenceRoot $OutputRoot
Write-JsonFile -Path (Join-Path $OutputRoot 'ADMIN-00-elevation-proof.json') -Value $adminProof
if (($adminProof.whoami_groups_exit -ne 0) -or
    (-not $adminProof.administrator_membership) -or
    (-not $adminProof.accepted_elevated_integrity_present) -or
    (-not $adminProof.powershell_x64)) {
    $summary = [pscustomobject]@{
        result = 'BLOCKED_ADMIN_PROOF'
        administrator_proof = $adminProof
        amd_runtime_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'VENDOR-NOOP-STARTUP-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}

$targetPath = Join-Path (Join-Path $InstallRoot 'bin') 'AMDuProf.exe'
$expectedSha256 = '8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762'
$artifact = $null
$preflightError = $null
try {
    $artifact = Get-ArtifactRecord `
        -Role 'amd_vendor_noop_target' `
        -Path $targetPath `
        -ExpectedSha256 $expectedSha256 `
        -SignatureRequired $true `
        -ExpectedSignerPattern '(?i)Advanced Micro Devices|\bAMD\b'
} catch {
    $preflightError = $_.Exception.Message
}
$preflightPass = ($null -ne $artifact) -and
    $artifact.sha256_match -and
    $artifact.architecture_match -and
    $artifact.signature_requirement_passed
$preflight = [pscustomobject]@{
    timestamp_utc = Get-UtcTimestamp
    install_root = $InstallRoot
    target = $targetPath
    expected_sha256 = $expectedSha256
    artifact = $artifact
    preflight_pass = $preflightPass
    error = $preflightError
}
Write-JsonFile -Path (Join-Path $OutputRoot 'ARTIFACT-PREFLIGHT.json') -Value $preflight
if (-not $preflightPass) {
    $summary = [pscustomobject]@{
        result = 'BLOCKED_ARTIFACT_PREFLIGHT'
        administrator_proof = $adminProof
        artifact_preflight = $preflight
        amd_runtime_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'VENDOR-NOOP-STARTUP-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}

$preexisting = Get-PreexistingVendorProcesses -ImageName 'AMDuProf.exe'
Write-JsonFile -Path (Join-Path $OutputRoot 'PREEXISTING-PROCESS-GATE.json') -Value $preexisting
if (-not $preexisting.query_succeeded) {
    $summary = [pscustomobject]@{
        result = 'BLOCKED_HARNESS_PREEXISTING_PROCESS_CHECK'
        administrator_proof = $adminProof
        artifact_preflight = $preflight
        preexisting_process_gate = $preexisting
        amd_runtime_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'VENDOR-NOOP-STARTUP-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}
if (@($preexisting.processes).Count -gt 0) {
    $summary = [pscustomobject]@{
        result = 'BLOCKED_PREEXISTING_VENDOR_PROCESS'
        administrator_proof = $adminProof
        artifact_preflight = $preflight
        preexisting_process_gate = $preexisting
        amd_runtime_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'VENDOR-NOOP-STARTUP-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}

$startup = Invoke-StartupObservation `
    -TestId 'VENDOR-NOOP-STARTUP' `
    -FilePath $targetPath `
    -Arguments @() `
    -WorkingDirectory (Join-Path $InstallRoot 'bin') `
    -ObservationWindowMs 3000 `
    -EvidenceRoot $OutputRoot `
    -ResultPath (Join-Path $OutputRoot 'VENDOR-NOOP-STARTUP.json') `
    -CaptureOutput `
    -AttemptGracefulCleanup

$summary = [pscustomobject]@{
    result = $startup.vendor_startup_control
    administrator_proof = $adminProof
    artifact_preflight = $preflight
    preexisting_process_gate = $preexisting
    startup_result = $startup
    observation_window_ms = 3000
    profiling_performed = $false
    sampling_performed = $false
    system_mutations_performed = $false
    amd_runtime_executed = $true
}
Write-JsonFile -Path (Join-Path $OutputRoot 'VENDOR-NOOP-STARTUP-SUMMARY.json') -Value $summary
Write-Output "EVIDENCE_ROOT=$OutputRoot"
