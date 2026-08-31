[CmdletBinding()]
param(
    [string]$InstallRoot = 'D:\apps\AMDuProf',
    [string]$OutputRoot = (Join-Path $env:TEMP ('resource-timeline-amd-public-api-ab-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ')))
)

# Manual Administrator invocation only. This wrapper does not elevate, change
# PATH, change the current directory, or run until the user explicitly starts
# this script from an Administrator PowerShell.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-UtcTimestamp {
    (Get-Date).ToUniversalTime().ToString('o')
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
        [Parameter(Mandatory = $true)][string[]]$Arguments,
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
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        if (-not $timedOut) {
            $targetExitSigned = [int]$process.ExitCode
            $targetExitHex = '0x{0:X8}' -f ([uint32]$targetExitSigned)
        }
    } finally {
        if ($processStarted -and $process.HasExited -and -not $timedOut) {
            $targetExitSigned = [int]$process.ExitCode
            $targetExitHex = '0x{0:X8}' -f ([uint32]$targetExitSigned)
        }
        $process.Dispose()
    }

    Write-Utf8File -Path $stdoutPath -Text $stdout
    Write-Utf8File -Path $stderrPath -Text $stderr
    $finishedAt = Get-UtcTimestamp
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
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
        stdout_bytes = ([System.Text.Encoding]::UTF8.GetByteCount($stdout))
        stderr_bytes = ([System.Text.Encoding]::UTF8.GetByteCount($stderr))
        capture_complete = ((Test-Path -LiteralPath $stdoutPath) -and (Test-Path -LiteralPath $stderrPath))
        kill_tree_attempted = $killTreeAttempted
        kill_tree_succeeded = $killTreeSucceeded
        kill_tree_error = $killTreeError
        fallback_kill_attempted = $fallbackKillAttempted
        fallback_kill_error = $fallbackKillError
    }
    Write-JsonFile -Path $ResultPath -Value $result
    $result
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
    api_library = '9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A427'
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
