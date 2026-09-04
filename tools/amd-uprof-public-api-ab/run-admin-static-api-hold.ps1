[CmdletBinding()]
param(
    [string]$InstallRoot = 'D:\apps\AMDuProf',
    [string]$OutputRoot = (Join-Path $env:TEMP ('resource-timeline-amd-static-hold-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'))),
    [int]$TimeoutMs = 10000,
    [switch]$SyntheticTest
)

# Manual Administrator invocation only for the AMD path. This script never
# elevates, changes PATH/current-directory state outside child ProcessStartInfo,
# or runs an AMD target in SyntheticTest mode.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$holdFixtureName = 'amd-uprof-static-api-hold-fixture.exe'
$expectedHoldFixtureSha256 = 'B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676'
$expectedApiSha256 = '9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277'
$holdWindowMs = 3000

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

    $json = $Value | ConvertTo-Json -Depth 12
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

    # The actual AMD invocation has no arguments. This fallback also supports
    # the non-AMD PowerShell fixture commands used by SyntheticTest.
    $escaped = $Argument.Replace('\', '\\').Replace('"', '\"')
    if ($StartInfo.Arguments) {
        $StartInfo.Arguments += ' '
    }
    $StartInfo.Arguments += '"' + $escaped + '"'
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
        $bytes[$peOffset + 2] -ne 0x00 -or $bytes[$peOffset + 3] -ne 0x00) {
        throw "missing PE signature: $Path"
    }
    [BitConverter]::ToUInt16($bytes, $peOffset + 4)
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
    $machine = Get-PeMachine -Path $Path
    $subject = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
    $issuer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Issuer } else { $null }
    $signerMatches = (-not $SignatureRequired) -or (($subject -match $ExpectedSignerPattern) -or ($issuer -match $ExpectedSignerPattern))
    $signaturePassed = (-not $SignatureRequired) -or (($signature.Status.ToString() -eq 'Valid') -and $signerMatches)

    [pscustomobject]@{
        role = $Role
        path = $Path
        size = $item.Length
        sha256 = $actualSha256
        expected_sha256 = $ExpectedSha256
        sha256_match = ($actualSha256 -ieq $ExpectedSha256)
        machine = ('0x{0:X4}' -f $machine)
        architecture = if ($machine -eq 0x8664) { 'x64' } else { 'UNKNOWN' }
        architecture_match = ($machine -eq 0x8664)
        file_version = $item.VersionInfo.FileVersion
        product_version = $item.VersionInfo.ProductVersion
        signature_status = $signature.Status.ToString()
        signature_subject = $subject
        signature_issuer = $issuer
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
    $whoamiOutputLines = @(& $whoamiPath /groups 2>&1 | ForEach-Object { [string]$_ })
    $whoamiExit = [int]$LASTEXITCODE
    $whoamiOutput = $whoamiOutputLines -join [Environment]::NewLine
    $whoamiOutputPath = Join-Path $EvidenceRoot 'ADMIN-00-whoami-groups.txt'
    Write-Utf8File -Path $whoamiOutputPath -Text $whoamiOutput

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $integritySids = @(
        [regex]::Matches($whoamiOutput, 'S-1-16-\d+') |
            ForEach-Object { $_.Value } |
            Select-Object -Unique
    )
    $integrityLevels = @($integritySids | ForEach-Object { [int]($_ -replace '^S-1-16-', '') })
    $acceptedIntegrity = @('S-1-16-12288', 'S-1-16-16384', 'S-1-16-20480', 'S-1-16-28672')
    $acceptedPresent = @($integritySids | Where-Object { $acceptedIntegrity -contains $_ })

    [pscustomobject]@{
        test_id = 'ADMIN-00'
        timestamp_utc = Get-UtcTimestamp
        username = $identity.Name
        powershell_path = (Get-Process -Id $PID).Path
        powershell_x64 = [Environment]::Is64BitProcess
        current_directory = (Get-Location).Path
        whoami_executable = $whoamiPath
        whoami_groups_exit = $whoamiExit
        whoami_groups_output_path = $whoamiOutputPath
        whoami_groups_output = $whoamiOutput
        administrator_membership = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        parsed_integrity_sids = $integritySids
        parsed_integrity_levels = $integrityLevels
        accepted_elevated_integrity_sids = $acceptedPresent
        accepted_elevated_integrity_present = ($acceptedPresent.Count -gt 0)
        self_elevation_performed = $false
    }
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$TestId,
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][int]$ProcessTimeoutMs,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$ResultPath
    )

    $stdoutPath = Join-Path $EvidenceRoot ($TestId + '.stdout.txt')
    $stderrPath = Join-Path $EvidenceRoot ($TestId + '.stderr.txt')
    $startedAt = Get-UtcTimestamp
    $process = [System.Diagnostics.Process]::new()
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @($Arguments)) {
        Add-ProcessArgument -StartInfo $startInfo -Argument $argument
    }
    $process.StartInfo = $startInfo

    $processStarted = $false
    $timedOut = $false
    $targetPid = $null
    $targetExitSigned = $null
    $stdoutTask = $null
    $stderrTask = $null
    $stdout = ''
    $stderr = ''
    $harnessError = $null
    $killTreeAttempted = $false
    $killTreeSucceeded = $false
    $killTreeError = $null
    $fallbackKillAttempted = $false
    $fallbackKillSucceeded = $false
    $fallbackKillError = $null

    try {
        $processStarted = $process.Start()
        if (-not $processStarted) {
            throw "Process.Start returned false for $FilePath"
        }
        $targetPid = $process.Id
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($ProcessTimeoutMs)) {
            $timedOut = $true
            $killTreeAttempted = $true
            try {
                $process.Kill($true)
                $killTreeSucceeded = $true
            } catch {
                $killTreeError = $_.Exception.Message
                $fallbackKillAttempted = $true
                try {
                    $process.Kill()
                    $fallbackKillSucceeded = $true
                } catch {
                    $fallbackKillError = $_.Exception.Message
                }
            }
            [void]$process.WaitForExit(2000)
        } else {
            $targetExitSigned = [int]$process.ExitCode
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
                if ([string]::IsNullOrWhiteSpace($harnessError)) {
                    $harnessError = $_.Exception.Message
                }
            }
        }
        if ($null -ne $stdoutTask) {
            try {
                $stdout = $stdoutTask.GetAwaiter().GetResult()
            } catch {
                if ([string]::IsNullOrWhiteSpace($harnessError)) {
                    $harnessError = $_.Exception.Message
                }
            }
        }
        if ($null -ne $stderrTask) {
            try {
                $stderr = $stderrTask.GetAwaiter().GetResult()
            } catch {
                if ([string]::IsNullOrWhiteSpace($harnessError)) {
                    $harnessError = $_.Exception.Message
                }
            }
        }
        $process.Dispose()
    }

    $targetExitHex = $null
    if ($null -ne $targetExitSigned) {
        try {
            $targetExitHex = Convert-ExitCodeToHex -ExitCode $targetExitSigned
        } catch {
            if ([string]::IsNullOrWhiteSpace($harnessError)) {
                $harnessError = $_.Exception.Message
            }
        }
    }

    $stdoutPersisted = $false
    $stderrPersisted = $false
    try {
        Write-Utf8File -Path $stdoutPath -Text $stdout
        $stdoutPersisted = $true
    } catch {
        if ([string]::IsNullOrWhiteSpace($harnessError)) {
            $harnessError = $_.Exception.Message
        }
    }
    try {
        Write-Utf8File -Path $stderrPath -Text $stderr
        $stderrPersisted = $true
    } catch {
        if ([string]::IsNullOrWhiteSpace($harnessError)) {
            $harnessError = $_.Exception.Message
        }
    }

    $finishedAt = Get-UtcTimestamp
    $targetProcessFailed = ($null -ne $targetExitSigned) -and ($targetExitSigned -ne 0)
    $targetProcessStatus = if ($timedOut) {
        'TARGET_TIMEOUT'
    } elseif ($targetProcessFailed) {
        'TARGET_PROCESS_FAILED'
    } elseif ($null -ne $targetExitSigned) {
        'TARGET_SUCCEEDED'
    } else {
        'TARGET_EXIT_NOT_AVAILABLE'
    }
    $result = [pscustomobject]@{
        test_id = $TestId
        process_started = $processStarted
        target_pid = $targetPid
        executable = $FilePath
        arguments = @($Arguments)
        working_directory = $WorkingDirectory
        started_at_utc = $startedAt
        finished_at_utc = $finishedAt
        timeout_ms = $ProcessTimeoutMs
        timeout = $timedOut
        target_exit_signed = $targetExitSigned
        target_exit_hex = $targetExitHex
        result_path = $ResultPath
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
        stdout_bytes = [System.Text.Encoding]::UTF8.GetByteCount($stdout)
        stderr_bytes = [System.Text.Encoding]::UTF8.GetByteCount($stderr)
        stdout_persisted = $stdoutPersisted
        stderr_persisted = $stderrPersisted
        capture_complete = ($stdoutPersisted -and $stderrPersisted)
        target_process_failed = $targetProcessFailed
        target_process_status = $targetProcessStatus
        harness_failed = (-not [string]::IsNullOrWhiteSpace($harnessError))
        harness_error = $harnessError
        kill_tree_attempted = $killTreeAttempted
        kill_tree_succeeded = $killTreeSucceeded
        kill_tree_error = $killTreeError
        fallback_kill_attempted = $fallbackKillAttempted
        fallback_kill_succeeded = $fallbackKillSucceeded
        fallback_kill_error = $fallbackKillError
    }
    Write-JsonFile -Path $ResultPath -Value $result
    $result
}

function Invoke-SyntheticValidation {
    param(
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    New-Item -ItemType Directory -Path $EvidenceRoot -Force | Out-Null
    $syntheticPowerShell = (Get-Process -Id $PID).Path
    $workingDirectory = (Get-Location).Path

    $negativeCommand = "[Console]::Out.WriteLine('SYNTHETIC_NEGATIVE_STDOUT_MARKER'); [Console]::Error.WriteLine('SYNTHETIC_NEGATIVE_STDERR_MARKER'); exit -1"
    $holdCommand = "[Console]::Out.WriteLine('HOLD_FIXTURE_MAIN_REACHED=true'); Start-Sleep -Milliseconds 3000; [Console]::Out.WriteLine('HOLD_FIXTURE_BEFORE_RETURN=true'); exit 0"
    $emptyCommand = 'exit 0'
    $timeoutCommand = 'Start-Sleep -Seconds 30'

    $negative = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-NEGATIVE-EXIT' `
        -FilePath $syntheticPowerShell `
        -Arguments @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $negativeCommand) `
        -WorkingDirectory $workingDirectory `
        -ProcessTimeoutMs 5000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-NEGATIVE-EXIT.json')
    $hold = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-HOLD' `
        -FilePath $syntheticPowerShell `
        -Arguments @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $holdCommand) `
        -WorkingDirectory $workingDirectory `
        -ProcessTimeoutMs 10000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-HOLD.json')
    $empty = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-EMPTY-OUTPUT' `
        -FilePath (Join-Path $env:SystemRoot 'System32\cmd.exe') `
        -Arguments @('/d', '/c', $emptyCommand) `
        -WorkingDirectory $workingDirectory `
        -ProcessTimeoutMs 5000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-EMPTY-OUTPUT.json')
    $emptyArguments = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-EMPTY-ARGUMENTS' `
        -FilePath (Join-Path $env:SystemRoot 'System32\whoami.exe') `
        -Arguments @() `
        -WorkingDirectory $workingDirectory `
        -ProcessTimeoutMs 5000 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-EMPTY-ARGUMENTS.json')
    $timeout = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-TIMEOUT' `
        -FilePath $syntheticPowerShell `
        -Arguments @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $timeoutCommand) `
        -WorkingDirectory $workingDirectory `
        -ProcessTimeoutMs 250 `
        -EvidenceRoot $EvidenceRoot `
        -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-TIMEOUT.json')

    $negativeStdout = [System.IO.File]::ReadAllText($negative.stdout_path)
    $negativeStderr = [System.IO.File]::ReadAllText($negative.stderr_path)
    $holdStdout = [System.IO.File]::ReadAllText($hold.stdout_path)
    $emptyStdout = [System.IO.File]::ReadAllText($empty.stdout_path)
    $emptyStderr = [System.IO.File]::ReadAllText($empty.stderr_path)
    $holdElapsed = ([DateTimeOffset]::Parse($hold.finished_at_utc) - [DateTimeOffset]::Parse($hold.started_at_utc)).TotalMilliseconds
    $syntheticExecutables = @($negative, $hold, $empty, $emptyArguments, $timeout) | ForEach-Object { $_.executable }
    $syntheticPathsHaveNoAmd = @($syntheticExecutables | Where-Object { $_ -match '(?i)AMDuProf|AMDPowerProfile' }).Count -eq 0

    $summary = [pscustomobject]@{
        result = if (
            (Convert-ExitCodeToHex -ExitCode 0) -eq '0x00000000' -and
            (Convert-ExitCodeToHex -ExitCode 1) -eq '0x00000001' -and
            (Convert-ExitCodeToHex -ExitCode -1) -eq '0xFFFFFFFF' -and
            $negative.process_started -and (-not $negative.timeout) -and ($negative.target_exit_signed -eq -1) -and
            ($negative.target_exit_hex -eq '0xFFFFFFFF') -and $negative.capture_complete -and
            $negativeStdout.Contains('SYNTHETIC_NEGATIVE_STDOUT_MARKER') -and
            $negativeStderr.Contains('SYNTHETIC_NEGATIVE_STDERR_MARKER') -and
            (Test-Path -LiteralPath $negative.result_path) -and
            $hold.process_started -and (-not $hold.timeout) -and ($hold.target_exit_signed -eq 0) -and
            $hold.capture_complete -and $holdStdout.Contains('HOLD_FIXTURE_MAIN_REACHED=true') -and
            $holdStdout.Contains('HOLD_FIXTURE_BEFORE_RETURN=true') -and $holdElapsed -ge 2800 -and
            $empty.process_started -and (-not $empty.timeout) -and ($empty.target_exit_signed -eq 0) -and
            $empty.capture_complete -and ($emptyStdout.Length -eq 0) -and ($emptyStderr.Length -eq 0) -and
            $emptyArguments.process_started -and (-not $emptyArguments.timeout) -and
            ($emptyArguments.arguments.Count -eq 0) -and $emptyArguments.capture_complete -and
            $timeout.process_started -and $timeout.timeout -and
            ($timeout.kill_tree_attempted -or $timeout.fallback_kill_attempted) -and
            (-not $timeout.harness_failed) -and $syntheticPathsHaveNoAmd
        ) { 'SYNTHETIC_REGRESSION_PASS' } else { 'SYNTHETIC_REGRESSION_FAIL' }
        amd_runtime_executed = $false
        no_amd_executable_or_dll_started = $syntheticPathsHaveNoAmd
        exit_code_conversion = [pscustomobject]@{
            zero = Convert-ExitCodeToHex -ExitCode 0
            one = Convert-ExitCodeToHex -ExitCode 1
            negative_one = Convert-ExitCodeToHex -ExitCode -1
        }
        negative_exit = $negative
        hold_markers = $hold
        hold_elapsed_ms = $holdElapsed
        empty_output = $empty
        empty_arguments = $emptyArguments
        timeout_cleanup = $timeout
    }
    Write-JsonFile -Path (Join-Path $EvidenceRoot 'SYNTHETIC-REGRESSION-SUMMARY.json') -Value $summary
    if ($summary.result -ne 'SYNTHETIC_REGRESSION_PASS') {
        throw 'synthetic static-hold wrapper regression failed'
    }
    Write-Output "EVIDENCE_ROOT=$EvidenceRoot"
    Write-Output 'SYNTHETIC_REGRESSION=PASS'
}

New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null

if ($SyntheticTest) {
    Invoke-SyntheticValidation -EvidenceRoot $OutputRoot
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
    Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}

$fixturePath = Join-Path $PSScriptRoot ('target\release\' + $holdFixtureName)
$apiPath = Join-Path $InstallRoot 'bin\AMDPowerProfileAPI.dll'
$artifactRows = @()
$preflightError = $null
try {
    $artifactRows += Get-ArtifactRecord `
        -Role 'repository_static_api_hold_fixture' `
        -Path $fixturePath `
        -ExpectedSha256 $expectedHoldFixtureSha256 `
        -SignatureRequired $false `
        -ExpectedSignerPattern '(?i)Advanced Micro Devices|\bAMD\b'
    $artifactRows += Get-ArtifactRecord `
        -Role 'amd_vendor_api_dll' `
        -Path $apiPath `
        -ExpectedSha256 $expectedApiSha256 `
        -SignatureRequired $true `
        -ExpectedSignerPattern '(?i)Advanced Micro Devices|\bAMD\b'
} catch {
    $preflightError = $_.Exception.Message
}
$preflight = [pscustomobject]@{
    timestamp_utc = Get-UtcTimestamp
    install_root = $InstallRoot
    artifacts = $artifactRows
    all_sha_match = (@($artifactRows).Count -eq 2) -and (@($artifactRows | Where-Object { -not $_.sha256_match }).Count -eq 0)
    all_x64 = (@($artifactRows).Count -eq 2) -and (@($artifactRows | Where-Object { -not $_.architecture_match }).Count -eq 0)
    required_signatures_pass = (@($artifactRows | Where-Object { $_.signature_required -and -not $_.signature_requirement_passed }).Count -eq 0)
    preflight_pass = $false
    error = $preflightError
}
$preflight.preflight_pass = $preflight.all_sha_match -and $preflight.all_x64 -and $preflight.required_signatures_pass
Write-JsonFile -Path (Join-Path $OutputRoot 'ARTIFACT-PREFLIGHT.json') -Value $preflight
if (-not $preflight.preflight_pass) {
    $summary = [pscustomobject]@{
        result = 'BLOCKED_ARTIFACT_PREFLIGHT'
        administrator_proof = $adminProof
        artifact_preflight = $preflight
        amd_runtime_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}

$targetResult = Invoke-CapturedProcess `
    -TestId 'STATIC-HOLD' `
    -FilePath $fixturePath `
    -Arguments @() `
    -WorkingDirectory (Join-Path $InstallRoot 'bin') `
    -ProcessTimeoutMs $TimeoutMs `
    -EvidenceRoot $OutputRoot `
    -ResultPath (Join-Path $OutputRoot 'STATIC-HOLD.json')
$summary = [pscustomobject]@{
    result = 'CAPTURE_COMPLETE_ANALYZE_RAW_EVIDENCE'
    evidence_root = $OutputRoot
    administrator_proof = $adminProof
    artifact_preflight = $preflight
    hold_window_ms = $holdWindowMs
    timeout_ms = $TimeoutMs
    target_result = $targetResult
    profiling_performed = $false
    sampling_performed = $false
    amd_api_called_from_fixture_main = $false
    system_mutations_performed = $false
    amd_runtime_executed = $true
}
Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-SUMMARY.json') -Value $summary
Write-Output "EVIDENCE_ROOT=$OutputRoot"
