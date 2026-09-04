[CmdletBinding()]
param(
    [string]$InstallRoot = 'D:\apps\AMDuProf',
    [string]$OutputRoot = (Join-Path $env:TEMP ('resource-timeline-amd-public-api-ab-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'))),
    [switch]$A1Only,
    [switch]$RunB1,
    [switch]$SyntheticTest
)

# Manual Administrator invocation only. This wrapper does not elevate, change
# PATH, change the current directory, or run until the user explicitly starts
# this script from an Administrator PowerShell.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($A1Only -and $RunB1) {
    throw 'A1Only and RunB1 are mutually exclusive'
}
if (-not $SyntheticTest -and -not $A1Only -and -not $RunB1) {
    throw 'specify -A1Only for the repair rerun or -RunB1 for an explicitly authorized full A/B'
}

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

    Write-Utf8File -Path $Path -Text ($Value | ConvertTo-Json -Depth 12)
}

function Get-ArtifactRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Role,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256,
        [Parameter(Mandatory = $true)][bool]$SignatureRequired
    )

    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $actualSha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    $signaturePassed = (-not $SignatureRequired) -or ($signature.Status.ToString() -eq 'Valid')

    [pscustomobject]@{
        role = $Role
        path = $Path
        size = $item.Length
        sha256 = $actualSha256
        expected_sha256 = $ExpectedSha256
        sha256_match = ($actualSha256 -eq $ExpectedSha256)
        architecture = 'x64 (PE machine 0x8664; established before this run)'
        signature_status = $signature.Status.ToString()
        signature_subject = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
        signature_required = $SignatureRequired
        signature_requirement_passed = $signaturePassed
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

    # The current host exposes ArgumentList. This fallback keeps the wrapper
    # usable on older PowerShell/.NET hosts for the fixed, quoted paths here.
    $escaped = $Argument.Replace('\', '\\').Replace('"', '\"')
    if ($StartInfo.Arguments) {
        $StartInfo.Arguments += ' '
    }
    $StartInfo.Arguments += '"' + $escaped + '"'
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$TestId,
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][int]$TimeoutMs,
        [Parameter(Mandatory = $true)][string]$ResultPath
    )

    $stdoutPath = Join-Path $OutputRoot ($TestId + '.stdout.txt')
    $stderrPath = Join-Path $OutputRoot ($TestId + '.stderr.txt')
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
    $killTreeAttempted = $false
    $killTreeSucceeded = $false
    $killTreeError = $null
    $fallbackKillAttempted = $false
    $fallbackKillError = $null
    $stdout = ''
    $stderr = ''
    $targetExitSigned = $null
    $targetExitHex = $null
    $targetPid = $null
    $stdoutTask = $null
    $stderrTask = $null
    $harnessError = $null

    try {
        $processStarted = $process.Start()
        if (-not $processStarted) {
            throw "Process.Start returned false for $FilePath"
        }
        $targetPid = $process.Id
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMs)) {
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
                } catch {
                    $fallbackKillError = $_.Exception.Message
                }
            }
        }
        $process.WaitForExit()
        if (-not $timedOut) {
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

    # A target exit, including a negative exit, is data rather than a harness
    # exception. Compute its representation only after the raw streams have
    # been captured and keep persistence independent from that conversion.
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
        arguments = $Arguments
        working_directory = $WorkingDirectory
        started_at_utc = $startedAt
        finished_at_utc = $finishedAt
        timeout_ms = $TimeoutMs
        timeout = $timedOut
        target_exit_signed = $targetExitSigned
        target_exit_hex = $targetExitHex
        result_path = $ResultPath
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
        stdout_bytes = ([System.Text.Encoding]::UTF8.GetByteCount($stdout))
        stderr_bytes = ([System.Text.Encoding]::UTF8.GetByteCount($stderr))
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
        fallback_kill_error = $fallbackKillError
    }
    Write-JsonFile -Path $ResultPath -Value $result
    $result
}

if ($SyntheticTest) {
    New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
    $exitCodeConversionPassed = (
        (Convert-ExitCodeToHex -ExitCode 0) -eq '0x00000000' -and
        (Convert-ExitCodeToHex -ExitCode 1) -eq '0x00000001' -and
        (Convert-ExitCodeToHex -ExitCode -1) -eq '0xFFFFFFFF'
    )
    $syntheticExecutable = (Get-Process -Id $PID).Path
    $syntheticCommand = "[Console]::Out.WriteLine('SYNTHETIC_STDOUT_MARKER'); [Console]::Error.WriteLine('SYNTHETIC_STDERR_MARKER'); exit -1"
    $synthetic = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-NEGATIVE-EXIT' `
        -FilePath $syntheticExecutable `
        -Arguments @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $syntheticCommand) `
        -WorkingDirectory (Get-Location).Path `
        -TimeoutMs 20000 `
        -ResultPath (Join-Path $OutputRoot 'SYNTHETIC-NEGATIVE-EXIT.json')
    $syntheticStdout = Get-Content -LiteralPath $synthetic.stdout_path -Raw
    $syntheticStderr = Get-Content -LiteralPath $synthetic.stderr_path -Raw

    $zeroExitCommand = "[Console]::Out.WriteLine('SYNTHETIC_ZERO_STDOUT_MARKER'); exit 0"
    $zeroExit = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-ZERO-EXIT' `
        -FilePath $syntheticExecutable `
        -Arguments @('-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $zeroExitCommand) `
        -WorkingDirectory (Get-Location).Path `
        -TimeoutMs 20000 `
        -ResultPath (Join-Path $OutputRoot 'SYNTHETIC-ZERO-EXIT.json')
    $zeroExitStdout = Get-Content -LiteralPath $zeroExit.stdout_path -Raw
    $zeroExitStderr = [System.IO.File]::ReadAllText($zeroExit.stderr_path)

    $whoamiPath = Join-Path $env:SystemRoot 'System32\whoami.exe'
    $emptyArguments = Invoke-CapturedProcess `
        -TestId 'SYNTHETIC-EMPTY-ARGUMENTS' `
        -FilePath $whoamiPath `
        -Arguments @() `
        -WorkingDirectory (Get-Location).Path `
        -TimeoutMs 20000 `
        -ResultPath (Join-Path $OutputRoot 'SYNTHETIC-EMPTY-ARGUMENTS.json')

    $syntheticPassed = (
        $synthetic.process_started -and
        (-not $synthetic.timeout) -and
        ($synthetic.target_exit_signed -eq -1) -and
        ($synthetic.target_exit_hex -eq '0xFFFFFFFF') -and
        $synthetic.capture_complete -and
        $syntheticStdout.Contains('SYNTHETIC_STDOUT_MARKER') -and
        $syntheticStderr.Contains('SYNTHETIC_STDERR_MARKER') -and
        (Test-Path -LiteralPath $synthetic.result_path)
    )
    $zeroExitPassed = (
        $zeroExit.process_started -and
        (-not $zeroExit.timeout) -and
        ($zeroExit.target_exit_signed -eq 0) -and
        ($zeroExit.target_exit_hex -eq '0x00000000') -and
        $zeroExit.capture_complete -and
        $zeroExitStdout.Contains('SYNTHETIC_ZERO_STDOUT_MARKER') -and
        ($zeroExitStderr.Length -eq 0) -and
        (Test-Path -LiteralPath $zeroExit.result_path)
    )
    $emptyArgumentsPassed = (
        $emptyArguments.process_started -and
        (-not $emptyArguments.timeout) -and
        ($emptyArguments.target_exit_signed -eq 0) -and
        $emptyArguments.capture_complete -and
        (Test-Path -LiteralPath $emptyArguments.result_path)
    )
    $summary = [pscustomobject]@{
        result = if ($exitCodeConversionPassed -and $syntheticPassed -and $zeroExitPassed -and $emptyArgumentsPassed) { 'SYNTHETIC_REGRESSION_PASS' } else { 'SYNTHETIC_REGRESSION_FAIL' }
        exit_code_conversion = [pscustomobject]@{
            zero = Convert-ExitCodeToHex -ExitCode 0
            one = Convert-ExitCodeToHex -ExitCode 1
            negative_one = Convert-ExitCodeToHex -ExitCode -1
            passed = $exitCodeConversionPassed
        }
        synthetic_negative_exit = $synthetic
        synthetic_zero_exit = $zeroExit
        synthetic_empty_arguments = $emptyArguments
        amd_runtime_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'SYNTHETIC-REGRESSION-SUMMARY.json') -Value $summary
    if (-not ($exitCodeConversionPassed -and $syntheticPassed -and $zeroExitPassed -and $emptyArgumentsPassed)) {
        throw 'synthetic wrapper regression failed'
    }
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    Write-Output 'SYNTHETIC_REGRESSION=PASS'
    return
}

New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
$startedAt = Get-UtcTimestamp
$whoamiPath = Join-Path $env:SystemRoot 'System32\whoami.exe'
$whoamiOutput = @(& $whoamiPath /groups 2>&1 | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
$whoamiExit = $LASTEXITCODE
$whoamiPathOut = Join-Path $OutputRoot 'ADMIN-00-whoami-groups.txt'
Write-Utf8File -Path $whoamiPathOut -Text $whoamiOutput
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
$integritySids = @([regex]::Matches($whoamiOutput, 'S-1-16-(?:8192|8448|12288|16384|20480|28672)') | ForEach-Object { $_.Value } | Select-Object -Unique)
$acceptedIntegrity = @('S-1-16-12288', 'S-1-16-16384', 'S-1-16-20480', 'S-1-16-28672')
$elevatedIntegrity = @($integritySids | Where-Object { $acceptedIntegrity -contains $_ }).Count -gt 0
$adminProof = [pscustomobject]@{
    test_id = 'ADMIN-00'
    timestamp_utc = $startedAt
    username = [Security.Principal.WindowsIdentity]::GetCurrent().Name
    powershell_path = (Get-Process -Id $PID).Path
    powershell_x64 = [Environment]::Is64BitProcess
    current_directory = (Get-Location).Path
    whoami_executable = $whoamiPath
    whoami_groups_exit = $whoamiExit
    whoami_groups_output_path = $whoamiPathOut
    whoami_groups_output = $whoamiOutput
    administrator_membership = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    parsed_integrity_sids = $integritySids
    accepted_elevated_integrity_present = $elevatedIntegrity
    self_elevation_performed = $false
}
Write-JsonFile -Path (Join-Path $OutputRoot 'ADMIN-00-elevation-proof.json') -Value $adminProof
if (($whoamiExit -ne 0) -or (-not $adminProof.administrator_membership) -or (-not $adminProof.accepted_elevated_integrity_present) -or (-not $adminProof.powershell_x64)) {
    throw 'ADMIN-00 failed: run this script from an elevated x64 Administrator PowerShell'
}

$staticPath = Join-Path $PSScriptRoot 'target\release\amd-uprof-static-api-load-fixture.exe'
$dynamicPath = Join-Path $PSScriptRoot 'target\release\amd-uprof-dynamic-api-load-fixture.exe'
$apiPath = Join-Path $InstallRoot 'bin\AMDPowerProfileAPI.dll'
$expected = [ordered]@{
    static_fixture = '9FAC63BD6B1FF1888DFFC8736F4152B972164DDAE8E1369584A53C1705354F53'
    dynamic_fixture = '2111185AA7E9F162D864D4F8E9C72E17B1769D94A0A09B00543876877F36416A'
    api_library = '9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277'
}
$artifactRows = @(
    Get-ArtifactRecord -Role 'repository_static_fixture' -Path $staticPath -ExpectedSha256 $expected.static_fixture -SignatureRequired $false
    Get-ArtifactRecord -Role 'repository_dynamic_fixture' -Path $dynamicPath -ExpectedSha256 $expected.dynamic_fixture -SignatureRequired $false
    Get-ArtifactRecord -Role 'amd_vendor_api_dll' -Path $apiPath -ExpectedSha256 $expected.api_library -SignatureRequired $true
)
$preflight = [pscustomobject]@{
    timestamp_utc = Get-UtcTimestamp
    install_root = $InstallRoot
    artifacts = $artifactRows
    all_sha_match = (@($artifactRows | Where-Object { -not $_.sha256_match }).Count -eq 0)
    all_x64 = $true
    required_signatures_pass = (@($artifactRows | Where-Object { $_.signature_required -and -not $_.signature_requirement_passed }).Count -eq 0)
}
$preflight | Add-Member -NotePropertyName preflight_pass -NotePropertyValue ($preflight.all_sha_match -and $preflight.all_x64 -and $preflight.required_signatures_pass)
Write-JsonFile -Path (Join-Path $OutputRoot 'ARTIFACT-PREFLIGHT.json') -Value $preflight
if (-not $preflight.preflight_pass) {
    throw 'artifact preflight failed'
}

$staticResult = Invoke-CapturedProcess `
    -TestId 'A1-STATIC' `
    -FilePath $staticPath `
    -Arguments @() `
    -WorkingDirectory (Join-Path $InstallRoot 'bin') `
    -TimeoutMs 20000 `
    -ResultPath (Join-Path $OutputRoot 'A1-STATIC.json')
$staticStdout = Get-Content -LiteralPath $staticResult.stdout_path -Raw
if ($staticResult.timeout -or ($staticResult.target_exit_signed -ne 0) -or ($staticStdout -notmatch 'STATIC_FIXTURE_MAIN_REACHED=true')) {
    $summary = [pscustomobject]@{
        result = 'STATIC_CONTROL_INVALID'
        static_result = $staticResult
        dynamic_result = $null
        evidence_root = $OutputRoot
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'ADMIN-MINIMAL-AB-SUMMARY.json') -Value $summary
    throw 'A1 static control did not reach main and exit 0; dynamic control was not run'
}

if ($A1Only) {
    $summary = [pscustomobject]@{
        result = 'A1_ONLY_CAPTURE_COMPLETE'
        evidence_root = $OutputRoot
        administrator_proof = $adminProof
        artifact_preflight = $preflight
        static_result = $staticResult
        dynamic_result = $null
        profiling_performed = $false
        sampling_performed = $false
        system_mutations_performed = $false
        b1_executed = $false
    }
    Write-JsonFile -Path (Join-Path $OutputRoot 'ADMIN-MINIMAL-AB-SUMMARY.json') -Value $summary
    Write-Output "EVIDENCE_ROOT=$OutputRoot"
    return
}

$dynamicResult = Invoke-CapturedProcess `
    -TestId 'B1-DYNAMIC' `
    -FilePath $dynamicPath `
    -Arguments @($apiPath) `
    -WorkingDirectory (Join-Path $InstallRoot 'bin') `
    -TimeoutMs 20000 `
    -ResultPath (Join-Path $OutputRoot 'B1-DYNAMIC.json')
$summary = [pscustomobject]@{
    result = 'CAPTURE_COMPLETE_ANALYZE_RAW_EVIDENCE'
    evidence_root = $OutputRoot
    administrator_proof = $adminProof
    artifact_preflight = $preflight
    static_result = $staticResult
    dynamic_result = $dynamicResult
    profiling_performed = $false
    sampling_performed = $false
    system_mutations_performed = $false
}
Write-JsonFile -Path (Join-Path $OutputRoot 'ADMIN-MINIMAL-AB-SUMMARY.json') -Value $summary
Write-Output "EVIDENCE_ROOT=$OutputRoot"
