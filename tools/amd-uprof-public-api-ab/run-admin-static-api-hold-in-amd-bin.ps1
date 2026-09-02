[CmdletBinding()]
param(
    [string]$OutputRoot = (Join-Path $env:TEMP ('resource-timeline-amd-directory-confirmation-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'))),
    [int]$TimeoutMs = 10000,
    [switch]$SyntheticTest,
    [switch]$StaticPreflightOnly
)

# Normal execution is for one manually launched Administrator PowerShell.
# SyntheticTest and StaticPreflightOnly never start an AMD executable or DLL.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$installRoot = 'D:\apps\AMDuProf'
$binDirectory = Join-Path $installRoot 'bin'
$sourceFixturePath = Join-Path $PSScriptRoot 'target\release\amd-uprof-static-api-hold-fixture.exe'
$destinationPath = Join-Path $binDirectory 'resource-timeline-amd-static-hold-confirm.exe'
$apiPath = Join-Path $binDirectory 'AMDPowerProfileAPI.dll'
$cxlPath = Join-Path $binDirectory 'CXLBaseTools.dll'
$expectedFixtureSha256 = 'B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676'
$expectedApiSha256 = '9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277'
$expectedCxlSha256 = '4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931'
$holdWindowMs = 3000
$amdSignerPattern = '(?i)Advanced Micro Devices|\bAMD\b'

function Get-UtcTimestamp { (Get-Date).ToUniversalTime().ToString('o') }

function Convert-ExitCodeToHex {
    param([Parameter(Mandatory = $true)][int]$ExitCode)
    $bytes = [BitConverter]::GetBytes([int]$ExitCode)
    '0x{0:X8}' -f [BitConverter]::ToUInt32($bytes, 0)
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    Write-Utf8File -Path $Path -Text ($Value | ConvertTo-Json -Depth 16)
}

function Add-ProcessArgument {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.ProcessStartInfo]$StartInfo,
        [Parameter(Mandatory = $true)][string]$Argument
    )
    if ($StartInfo.PSObject.Properties.Name -contains 'ArgumentList') {
        [void]$StartInfo.ArgumentList.Add($Argument)
        return
    }
    $escaped = $Argument.Replace('\', '\\').Replace('"', '\"')
    if ($StartInfo.Arguments) { $StartInfo.Arguments += ' ' }
    $StartInfo.Arguments += '"' + $escaped + '"'
}

function Get-CurrentPowerShellPath {
    try {
        $path = (Get-Process -Id $PID -ErrorAction Stop).Path
        if (-not [string]::IsNullOrWhiteSpace($path)) { return $path }
    } catch { }
    $fallback = Join-Path $PSHOME 'powershell.exe'
    if (Test-Path -LiteralPath $fallback) { return $fallback }
    Join-Path $PSHOME 'pwsh.exe'
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
    $process = [Diagnostics.Process]::new()
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $FilePath; $info.WorkingDirectory = $WorkingDirectory; $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    foreach ($argument in @($Arguments)) { Add-ProcessArgument -StartInfo $info -Argument $argument }
    $process.StartInfo = $info
    $started = $false; $timeout = $false; $targetPid = $null; $exitSigned = $null; $outTask = $null; $errTask = $null
    $stdout = ''; $stderr = ''; $outError = $null; $errError = $null; $harnessError = $null
    $killTreeAttempted = $false; $killTreeSucceeded = $false; $killTreeError = $null; $fallbackKillAttempted = $false; $fallbackKillSucceeded = $false; $fallbackKillError = $null
    try {
        $started = $process.Start()
        if (-not $started) { throw "Process.Start returned false for $FilePath" }
        $targetPid = $process.Id; $outTask = $process.StandardOutput.ReadToEndAsync(); $errTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($ProcessTimeoutMs)) {
            $timeout = $true; $killTreeAttempted = $true
            try { $process.Kill($true); $killTreeSucceeded = $true }
            catch {
                $killTreeError = $_.Exception.Message; $fallbackKillAttempted = $true
                try { $process.Kill(); $fallbackKillSucceeded = $true } catch { $fallbackKillError = $_.Exception.Message }
            }
            [void]$process.WaitForExit(2000)
        }
        if ($process.HasExited) { $exitSigned = [int]$process.ExitCode }
    } catch { $harnessError = $_.Exception.Message }
    finally {
        foreach ($stream in @([pscustomobject]@{ task = $outTask; name = 'stdout' }, [pscustomobject]@{ task = $errTask; name = 'stderr' })) {
            if ($null -eq $stream.task) { continue }
            try {
                if (-not $stream.task.Wait(3000)) { throw "$($stream.name) capture drain timed out" }
                if ($stream.name -eq 'stdout') { $stdout = [string]$stream.task.Result } else { $stderr = [string]$stream.task.Result }
            } catch { if ($stream.name -eq 'stdout') { $outError = $_.Exception.Message } else { $errError = $_.Exception.Message } }
        }
        $process.Dispose()
    }
    $exitHex = if ($null -ne $exitSigned) { Convert-ExitCodeToHex -ExitCode $exitSigned } else { $null }
    $stdoutPersisted = $false; $stderrPersisted = $false
    try { Write-Utf8File -Path $stdoutPath -Text $stdout; $stdoutPersisted = $true } catch { if (-not $harnessError) { $harnessError = $_.Exception.Message } }
    try { Write-Utf8File -Path $stderrPath -Text $stderr; $stderrPersisted = $true } catch { if (-not $harnessError) { $harnessError = $_.Exception.Message } }
    $finishedAt = Get-UtcTimestamp
    $captureComplete = $started -and $stdoutPersisted -and $stderrPersisted -and (-not $outError) -and (-not $errError)
    $targetFailed = ($null -ne $exitSigned) -and ($exitSigned -ne 0)
    $result = [pscustomobject]@{
        test_id = $TestId; process_started = $started; target_pid = $targetPid; executable = $FilePath; arguments = @($Arguments); working_directory = $WorkingDirectory
        started_at_utc = $startedAt; finished_at_utc = $finishedAt; timeout_ms = $ProcessTimeoutMs; timeout = $timeout; target_exit_signed = $exitSigned; target_exit_hex = $exitHex
        stdout_path = $stdoutPath; stderr_path = $stderrPath; stdout_bytes = [Text.Encoding]::UTF8.GetByteCount($stdout); stderr_bytes = [Text.Encoding]::UTF8.GetByteCount($stderr)
        stdout_persisted = $stdoutPersisted; stderr_persisted = $stderrPersisted; capture_complete = $captureComplete; target_process_failed = $targetFailed
        target_process_status = if ($timeout) { 'TARGET_TIMEOUT' } elseif ($targetFailed) { 'TARGET_PROCESS_FAILED' } elseif ($null -ne $exitSigned) { 'TARGET_SUCCEEDED' } else { 'TARGET_EXIT_NOT_AVAILABLE' }
        harness_failed = (-not [string]::IsNullOrWhiteSpace($harnessError)) -or (-not [string]::IsNullOrWhiteSpace($outError)) -or (-not [string]::IsNullOrWhiteSpace($errError)) -or (-not $captureComplete)
        harness_error = if ($harnessError) { $harnessError } elseif ($outError) { $outError } else { $errError }; stdout_capture_error = $outError; stderr_capture_error = $errError
        kill_tree_attempted = $killTreeAttempted; kill_tree_succeeded = $killTreeSucceeded; kill_tree_error = $killTreeError; fallback_kill_attempted = $fallbackKillAttempted; fallback_kill_succeeded = $fallbackKillSucceeded; fallback_kill_error = $fallbackKillError
    }
    Write-JsonFile -Path $ResultPath -Value $result
    $result
}

function Get-PeInfo {
    param([Parameter(Mandatory = $true)][string]$Path)
    $b = [IO.File]::ReadAllBytes($Path)
    if ($b.Length -lt 0x40 -or $b[0] -ne 0x4D -or $b[1] -ne 0x5A) { throw "not a PE image: $Path" }
    $pe = [BitConverter]::ToInt32($b, 0x3C)
    if ($pe -lt 0 -or $pe + 24 -gt $b.Length -or $b[$pe] -ne 0x50 -or $b[$pe + 1] -ne 0x45 -or $b[$pe + 2] -ne 0 -or $b[$pe + 3] -ne 0) { throw "invalid PE header: $Path" }
    $machine = [BitConverter]::ToUInt16($b, $pe + 4); $sectionCount = [BitConverter]::ToUInt16($b, $pe + 6); $optionalSize = [BitConverter]::ToUInt16($b, $pe + 20); $optional = $pe + 24; $magic = [BitConverter]::ToUInt16($b, $optional)
    $directory = $optional + $(if ($magic -eq 0x20B) { 112 } elseif ($magic -eq 0x10B) { 96 } else { throw "unsupported PE optional header: $Path" }); $importRva = [BitConverter]::ToUInt32($b, $directory + 8); $sectionTable = $optional + $optionalSize
    $sections = @()
    for ($i = 0; $i -lt $sectionCount; $i++) { $o = $sectionTable + ($i * 40); $end = $o; while ($end -lt $o + 8 -and $b[$end] -ne 0) { $end++ }; $sections += [pscustomobject]@{ va = [BitConverter]::ToUInt32($b, $o + 12); vs = [BitConverter]::ToUInt32($b, $o + 8); raw = [BitConverter]::ToUInt32($b, $o + 20); rs = [BitConverter]::ToUInt32($b, $o + 16) } }
    function Convert-Rva([uint32]$Rva) { foreach ($s in $sections) { $span = if ([uint64]$s.vs -gt [uint64]$s.rs) { [uint64]$s.vs } else { [uint64]$s.rs }; if ([uint64]$Rva -ge [uint64]$s.va -and [uint64]$Rva -lt ([uint64]$s.va + $span)) { return [int]([uint64]$s.raw + ([uint64]$Rva - [uint64]$s.va)) } }; $null }
    function Read-AsciiZ([int]$Offset) { $end = $Offset; while ($end -lt $b.Length -and $b[$end] -ne 0) { $end++ }; [Text.Encoding]::ASCII.GetString($b, $Offset, $end - $Offset) }
    $imports = New-Object System.Collections.Generic.List[string]
    if ($importRva -ne 0) { $importOffset = Convert-Rva $importRva; for ($i = 0; $i -lt 4096; $i++) { $o = $importOffset + ($i * 20); if ($o + 20 -gt $b.Length) { throw "truncated import table: $Path" }; $fields = @(0..4 | ForEach-Object { [BitConverter]::ToUInt32($b, $o + ($_ * 4)) }); if (@($fields | Where-Object { $_ -ne 0 }).Count -eq 0) { break }; [void]$imports.Add((Read-AsciiZ (Convert-Rva $fields[3]))) } }
    [pscustomobject]@{ machine = $machine; architecture = if ($machine -eq 0x8664) { 'x64' } else { 'UNKNOWN' }; subsystem = [BitConverter]::ToUInt16($b, $optional + 68); imports = @($imports.ToArray() | Sort-Object -Unique) }
}

function Get-Artifact {
    param([Parameter(Mandatory = $true)][string]$Role, [Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$ExpectedSha256, [Parameter(Mandatory = $true)][bool]$SignatureRequired)
    $item = Get-Item -LiteralPath $Path -ErrorAction Stop; $sig = Get-AuthenticodeSignature -LiteralPath $Path; $pe = Get-PeInfo -Path $Path; $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    $subject = if ($sig.SignerCertificate) { $sig.SignerCertificate.Subject } else { $null }; $issuer = if ($sig.SignerCertificate) { $sig.SignerCertificate.Issuer } else { $null }; $signerMatches = (-not $SignatureRequired) -or ($subject -match $amdSignerPattern) -or ($issuer -match $amdSignerPattern); $signaturePass = (-not $SignatureRequired) -or (($sig.Status.ToString() -eq 'Valid') -and $signerMatches)
    $apiImport = @($pe.imports | Where-Object { $_ -ieq 'AMDPowerProfileAPI.dll' }).Count -gt 0; $cxlImport = @($pe.imports | Where-Object { $_ -ieq 'CXLBaseTools.dll' }).Count -gt 0
    [pscustomobject]@{ role = $Role; path = $Path; size = $item.Length; sha256 = $hash; expected_sha256 = $ExpectedSha256; sha256_match = $hash -ieq $ExpectedSha256; machine = ('0x{0:X4}' -f $pe.machine); architecture = $pe.architecture; architecture_match = $pe.machine -eq 0x8664; subsystem = ('0x{0:X4}' -f $pe.subsystem); file_version = $item.VersionInfo.FileVersion; product_version = $item.VersionInfo.ProductVersion; signature_status = $sig.Status.ToString(); signature_subject = $subject; signature_issuer = $issuer; signature_required = $SignatureRequired; signer_matches_expected = $signerMatches; signature_requirement_passed = $signaturePass; direct_imports = @($pe.imports); direct_import_amdpowerprofileapi = $apiImport; direct_import_cxlbasetools = $cxlImport; static_import_assertion_passed = if ($Role -eq 'repository_static_api_hold_fixture') { $pe.machine -eq 0x8664 -and $apiImport -and (-not $cxlImport) } else { $true } }
}

function Get-AdminProof {
    param([Parameter(Mandatory = $true)][string]$EvidenceRoot)
    $whoami = Join-Path $env:SystemRoot 'System32\whoami.exe'; $lines = @(& $whoami /groups 2>&1 | ForEach-Object { [string]$_ }); $exit = [int]$LASTEXITCODE; $text = $lines -join [Environment]::NewLine; $whoamiOut = Join-Path $EvidenceRoot 'ADMIN-00-whoami-groups.txt'; Write-Utf8File -Path $whoamiOut -Text $text
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent(); $principal = [Security.Principal.WindowsPrincipal]::new($identity); $sids = @([regex]::Matches($text, 'S-1-16-\d+') | ForEach-Object { $_.Value } | Select-Object -Unique); $accepted = @('S-1-16-12288', 'S-1-16-16384', 'S-1-16-20480', 'S-1-16-28672')
    [pscustomobject]@{ test_id = 'ADMIN-00'; timestamp_utc = Get-UtcTimestamp; username = $identity.Name; powershell_path = Get-CurrentPowerShellPath; powershell_x64 = [Environment]::Is64BitProcess; current_directory = (Get-Location).Path; whoami_executable = $whoami; whoami_groups_exit = $exit; whoami_groups_output_path = $whoamiOut; whoami_groups_output = $text; parsed_integrity_sids = $sids; accepted_elevated_integrity_sids = @($sids | Where-Object { $accepted -contains $_ }); accepted_elevated_integrity_present = @($sids | Where-Object { $accepted -contains $_ }).Count -gt 0; administrator_membership = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator); self_elevation_performed = $false }
}

function Get-Markers {
    param([Parameter(Mandatory = $true)][AllowNull()][string]$Text)
    if ($null -eq $Text) { throw 'marker parser received null output' }
    [pscustomobject]@{ main = $Text.Contains('HOLD_FIXTURE_MAIN_REACHED=true'); before_return = $Text.Contains('HOLD_FIXTURE_BEFORE_RETURN=true') }
}

function Remove-ExactCopiedFile {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$ExpectedSha256)
    $exists = Test-Path -LiteralPath $Path -PathType Leaf; $current = $null; $status = 'NOT_PRESENT'; $error = $null
    if ($exists) { try { $current = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant(); if ($current -ine $ExpectedSha256) { $status = 'FAILED_EXACT_FILE_HASH_CHANGED' } else { Remove-Item -LiteralPath $Path -Force -ErrorAction Stop; $status = if (Test-Path -LiteralPath $Path -PathType Leaf) { 'FAILED_EXACT_DIAGNOSTIC_FILE_REMAINS' } else { 'REMOVED' } } } catch { $status = 'FAILED_EXACT_DIAGNOSTIC_FILE_REMAINS'; $error = $_.Exception.Message } }
    [pscustomobject]@{ path = $Path; expected_sha256 = $ExpectedSha256; exists_before = $exists; sha256_before = $current; status = $status; error = $error; exists_after = Test-Path -LiteralPath $Path -PathType Leaf }
}

function Get-Preflight {
    param([Parameter(Mandatory = $true)][string]$EvidenceRoot)
    $rows = @(); $error = $null
    try { $rows += Get-Artifact -Role 'repository_static_api_hold_fixture' -Path $sourceFixturePath -ExpectedSha256 $expectedFixtureSha256 -SignatureRequired $false; $rows += Get-Artifact -Role 'amd_vendor_api_dll' -Path $apiPath -ExpectedSha256 $expectedApiSha256 -SignatureRequired $true; $rows += Get-Artifact -Role 'amd_vendor_cxl_dll' -Path $cxlPath -ExpectedSha256 $expectedCxlSha256 -SignatureRequired $true } catch { $error = $_.Exception.Message }
    $source = $rows | Where-Object { $_.role -eq 'repository_static_api_hold_fixture' } | Select-Object -First 1; $sha = $rows.Count -eq 3 -and @($rows | Where-Object { -not $_.sha256_match }).Count -eq 0; $x64 = $rows.Count -eq 3 -and @($rows | Where-Object { -not $_.architecture_match }).Count -eq 0; $signatures = $rows.Count -eq 3 -and @($rows | Where-Object { $_.signature_required -and -not $_.signature_requirement_passed }).Count -eq 0; $imports = $null -ne $source -and $source.static_import_assertion_passed
    $preflight = [pscustomobject]@{ timestamp_utc = Get-UtcTimestamp; install_root = $installRoot; destination_directory = $binDirectory; destination_directory_exists = Test-Path -LiteralPath $binDirectory -PathType Container; source_fixture_path = $sourceFixturePath; destination_path = $destinationPath; artifacts = $rows; all_sha_match = $sha; all_x64 = $x64; required_signatures_pass = $signatures; source_static_import_assertion_pass = $imports; preflight_pass = $sha -and $x64 -and $signatures -and $imports; error = $error }
    Write-JsonFile -Path (Join-Path $EvidenceRoot 'ARTIFACT-PREFLIGHT.json') -Value $preflight; $preflight
}

function Invoke-SyntheticValidation {
    param([Parameter(Mandatory = $true)][string]$EvidenceRoot)
    New-Item -ItemType Directory -Path $EvidenceRoot -Force | Out-Null; $root = Join-Path ([IO.Path]::GetTempPath()) ('resource-timeline-directory-confirmation-synthetic-' + [guid]::NewGuid().ToString('N')); New-Item -ItemType Directory -Path $root -Force | Out-Null; $shell = Get-CurrentPowerShellPath
    $source = Join-Path $root 'source.txt'; $copy = Join-Path $root 'copy.txt'; $existing = Join-Path $root 'existing.txt'; $sibling = Join-Path $root 'sibling.txt'; Write-Utf8File -Path $source -Text 'synthetic source bytes'; [IO.File]::Copy($source, $copy, $false); $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToUpperInvariant(); $copyHash = (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash.ToUpperInvariant(); $t1 = $sourceHash -eq $copyHash
    Write-Utf8File -Path $existing -Text 'must not overwrite'; $old = [IO.File]::ReadAllText($existing); $t2 = (Test-Path -LiteralPath $existing) -and ([IO.File]::ReadAllText($existing) -eq $old); $parsed = Get-Markers -Text "HOLD_FIXTURE_MAIN_REACHED=true`r`nHOLD_FIXTURE_BEFORE_RETURN=true`r`n"; $t3 = $parsed.main -and $parsed.before_return
    $hold = Invoke-CapturedProcess -TestId 'SYNTHETIC-HOLD' -FilePath $shell -Arguments @('-NoLogo','-NoProfile','-NonInteractive','-Command',"[Console]::Out.WriteLine('HOLD_FIXTURE_MAIN_REACHED=true'); Start-Sleep -Milliseconds 3000; [Console]::Out.WriteLine('HOLD_FIXTURE_BEFORE_RETURN=true'); exit 0") -WorkingDirectory $root -ProcessTimeoutMs 10000 -EvidenceRoot $EvidenceRoot -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-HOLD.json')
    $negative = Invoke-CapturedProcess -TestId 'SYNTHETIC-NEGATIVE' -FilePath $shell -Arguments @('-NoLogo','-NoProfile','-NonInteractive','-Command',"[Console]::Out.WriteLine('NEGATIVE_STDOUT'); [Console]::Error.WriteLine('NEGATIVE_STDERR'); exit -1") -WorkingDirectory $root -ProcessTimeoutMs 5000 -EvidenceRoot $EvidenceRoot -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-NEGATIVE.json')
    $empty = Invoke-CapturedProcess -TestId 'SYNTHETIC-EMPTY' -FilePath $shell -Arguments @('-NoLogo','-NoProfile','-NonInteractive','-Command','exit 0') -WorkingDirectory $root -ProcessTimeoutMs 5000 -EvidenceRoot $EvidenceRoot -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-EMPTY.json')
    $timeout = Invoke-CapturedProcess -TestId 'SYNTHETIC-TIMEOUT' -FilePath $shell -Arguments @('-NoLogo','-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 30') -WorkingDirectory $root -ProcessTimeoutMs 250 -EvidenceRoot $EvidenceRoot -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-TIMEOUT.json')
    $holdText = [IO.File]::ReadAllText($hold.stdout_path); $negativeOut = [IO.File]::ReadAllText($negative.stdout_path); $negativeErr = [IO.File]::ReadAllText($negative.stderr_path); $emptyOut = [IO.File]::ReadAllText($empty.stdout_path); $emptyErr = [IO.File]::ReadAllText($empty.stderr_path); $holdMarkers = Get-Markers -Text $holdText; $elapsed = ([DateTimeOffset]::Parse($hold.finished_at_utc) - [DateTimeOffset]::Parse($hold.started_at_utc)).TotalMilliseconds
    $t4 = $hold.process_started -and (-not $hold.timeout) -and $hold.capture_complete -and ($hold.target_exit_signed -eq 0) -and $holdMarkers.main -and $holdMarkers.before_return -and ($elapsed -ge 2800); $t5 = $negative.process_started -and (-not $negative.timeout) -and $negative.capture_complete -and ($negative.target_exit_signed -eq -1) -and ($negative.target_exit_hex -eq '0xFFFFFFFF') -and $negativeOut.Contains('NEGATIVE_STDOUT') -and $negativeErr.Contains('NEGATIVE_STDERR'); $t6 = $empty.process_started -and (-not $empty.timeout) -and $empty.capture_complete -and ($empty.stdout_bytes -eq 0) -and ($empty.stderr_bytes -eq 0) -and ($emptyOut.Length -eq 0) -and ($emptyErr.Length -eq 0)
    $qpath = Join-Path $EvidenceRoot 'SYNTHETIC-QUALIFICATION-BEFORE-CLEANUP.json'; Write-JsonFile -Path $qpath -Value ([pscustomobject]@{ qualification = 'SYNTHETIC_PASS'; source_sha256 = $sourceHash; destination_sha256 = $copyHash }); $t7 = (Test-Path -LiteralPath $qpath) -and ((Get-Content -Raw -LiteralPath $qpath | ConvertFrom-Json).qualification -eq 'SYNTHETIC_PASS'); Write-Utf8File -Path $sibling -Text 'must remain'; $cleanup = Remove-ExactCopiedFile -Path $copy -ExpectedSha256 $sourceHash; $t8 = ($cleanup.status -eq 'REMOVED') -and (Test-Path -LiteralPath $sibling); $t9 = -not $cleanup.exists_after
    $rawPath = Join-Path $EvidenceRoot 'SYNTHETIC-RAW-BEFORE-PARSER-FAILURE.txt'; Write-Utf8File -Path $rawPath -Text 'raw survives'; $parserFailed = $false; try { [void](Get-Markers -Text $null) } catch { $parserFailed = $true }; $t10 = $parserFailed -and ([IO.File]::ReadAllText($rawPath) -eq 'raw survives'); $t11 = $timeout.process_started -and $timeout.timeout -and (-not $timeout.harness_failed) -and ($timeout.kill_tree_attempted -or $timeout.fallback_kill_attempted)
    $emptyArguments = Invoke-CapturedProcess -TestId 'SYNTHETIC-EMPTY-ARGUMENTS' -FilePath (Join-Path $env:SystemRoot 'System32\whoami.exe') -Arguments @() -WorkingDirectory $root -ProcessTimeoutMs 5000 -EvidenceRoot $EvidenceRoot -ResultPath (Join-Path $EvidenceRoot 'SYNTHETIC-EMPTY-ARGUMENTS.json')
    $summary = [pscustomobject]@{ result = if ($t1 -and $t2 -and $t3 -and $t4 -and $t5 -and $t6 -and $t7 -and $t8 -and $t9 -and $t10 -and $t11 -and $emptyArguments.process_started -and $emptyArguments.capture_complete) { 'SYNTHETIC_REGRESSION_PASS' } else { 'SYNTHETIC_REGRESSION_FAIL' }; amd_runtime_executed = $false; no_amd_executable_or_dll_started = $true; test_results = [pscustomobject]@{ T1_hash_equality = $t1; T2_preexisting_gate = $t2; T3_marker_parsing = $t3; T4_hold_success = $t4; T5_negative_exit = $t5; T6_empty_streams = $t6; T7_qualification_persistence = $t7; T8_exact_cleanup = $t8; T9_cleanup_verification = $t9; T10_parser_failure_raw_preserved = $t10; T11_timeout_behavior = $t11; empty_arguments = $emptyArguments.process_started -and $emptyArguments.capture_complete }; exit_code_conversion = [pscustomobject]@{ zero = Convert-ExitCodeToHex 0; one = Convert-ExitCodeToHex 1; negative_one = Convert-ExitCodeToHex -1 }; hold = $hold; negative = $negative; empty = $empty; empty_arguments_result = $emptyArguments; timeout = $timeout; cleanup = $cleanup; synthetic_root = $root }
    Write-JsonFile -Path (Join-Path $EvidenceRoot 'SYNTHETIC-REGRESSION-SUMMARY.json') -Value $summary; if ($summary.result -ne 'SYNTHETIC_REGRESSION_PASS') { throw 'synthetic directory-confirmation wrapper regression failed' }; Write-Output "EVIDENCE_ROOT=$EvidenceRoot"; Write-Output 'SYNTHETIC_REGRESSION=PASS'
}

New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
if ($SyntheticTest) { Invoke-SyntheticValidation -EvidenceRoot $OutputRoot; return }
if ($StaticPreflightOnly) { $p = Get-Preflight -EvidenceRoot $OutputRoot; Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-PREFLIGHT-ONLY-SUMMARY.json') -Value ([pscustomobject]@{ result = if ($p.preflight_pass) { 'STATIC_PREFLIGHT_PASS' } else { 'STATIC_PREFLIGHT_FAIL' }; amd_runtime_executed = $false; artifact_preflight = $p }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }

$admin = Get-AdminProof -EvidenceRoot $OutputRoot; Write-JsonFile -Path (Join-Path $OutputRoot 'ADMIN-00-elevation-proof.json') -Value $admin
if ($admin.whoami_groups_exit -ne 0 -or -not $admin.administrator_membership -or -not $admin.accepted_elevated_integrity_present -or -not $admin.powershell_x64) { Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = 'BLOCKED_ADMIN_PROOF'; administrator_proof = $admin; amd_runtime_executed = $false }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }
$preflight = Get-Preflight -EvidenceRoot $OutputRoot
if (-not $preflight.preflight_pass) { Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = 'BLOCKED_ARTIFACT_PREFLIGHT'; administrator_proof = $admin; artifact_preflight = $preflight; amd_runtime_executed = $false }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }
if (-not (Test-Path -LiteralPath $binDirectory -PathType Container)) { Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = 'BLOCKED_DESTINATION_DIRECTORY_MISSING'; destination_directory = $binDirectory; amd_runtime_executed = $false }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }
if (Test-Path -LiteralPath $destinationPath) { Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = 'BLOCKED_DESTINATION_ALREADY_EXISTS'; destination_path = $destinationPath; amd_runtime_executed = $false }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }

try { [IO.File]::Copy($sourceFixturePath, $destinationPath, $false); $destinationSha256 = (Get-FileHash -LiteralPath $destinationPath -Algorithm SHA256).Hash.ToUpperInvariant() } catch { Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = 'BLOCKED_COPY_FAILURE'; source_fixture_path = $sourceFixturePath; destination_path = $destinationPath; error = $_.Exception.Message; amd_runtime_executed = $false }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }
if ($destinationSha256 -ine $expectedFixtureSha256) { Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = 'BLOCKED_DESTINATION_HASH_MISMATCH_BEFORE_LAUNCH'; source_sha256 = $expectedFixtureSha256; destination_sha256 = $destinationSha256; destination_path = $destinationPath; amd_runtime_executed = $false }); Write-Output "EVIDENCE_ROOT=$OutputRoot"; return }

$target = Invoke-CapturedProcess -TestId 'STATIC-HOLD-IN-AMD-BIN' -FilePath $destinationPath -Arguments @() -WorkingDirectory $binDirectory -ProcessTimeoutMs $TimeoutMs -EvidenceRoot $OutputRoot -ResultPath (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN.capture.json')
$stdout = [IO.File]::ReadAllText($target.stdout_path); $stderr = [IO.File]::ReadAllText($target.stderr_path); $markerError = $null; $main = $false; $before = $false; try { $m = Get-Markers -Text $stdout; $main = $m.main; $before = $m.before_return } catch { $markerError = $_.Exception.Message }
$qualification = if ($target.harness_failed -or -not $target.capture_complete) { 'BLOCKED_HARNESS' } elseif ($target.timeout) { 'TARGET_TIMEOUT' } elseif ($main -and $before -and $target.target_exit_signed -eq 0) { 'PROCESS_DIRECTORY_RUNTIME_CONFIRMATION_PASS' } elseif ($main -and $before -and $target.target_exit_signed -eq -1) { 'STARTUP_GATE_BYPASSED_ADDITIONAL_SHUTDOWN_OR_OTHER_FAILURE' } elseif ($main -and -not $before) { 'DIRECTORY_POLICY_STARTUP_GATE_BYPASSED_ADDITIONAL_RUNTIME_FAILURE' } elseif (-not $main -and $target.target_exit_signed -eq -1) { 'DIRECTORY_CHANGE_NOT_SUFFICIENT' } else { 'INCONCLUSIVE' }
$qualificationRecord = [pscustomobject]@{ source_sha256 = $expectedFixtureSha256; destination_sha256 = $destinationSha256; byte_identical_copy = $destinationSha256 -ieq $expectedFixtureSha256; source_exe_path = $sourceFixturePath; destination_exe_path = $destinationPath; source_exe_directory = [IO.Path]::GetDirectoryName($sourceFixturePath); destination_exe_directory = $binDirectory; process_started = $target.process_started; target_pid = $target.target_pid; arguments = @(); working_directory = $binDirectory; started_at_utc = $target.started_at_utc; finished_at_utc = $target.finished_at_utc; duration_ms = ([DateTimeOffset]::Parse($target.finished_at_utc) - [DateTimeOffset]::Parse($target.started_at_utc)).TotalMilliseconds; timeout_ms = $TimeoutMs; timeout = $target.timeout; target_exit_signed = $target.target_exit_signed; target_exit_hex = $target.target_exit_hex; stdout_path = $target.stdout_path; stderr_path = $target.stderr_path; stdout_bytes = $target.stdout_bytes; stderr_bytes = $target.stderr_bytes; main_marker_present = $main; before_return_marker_present = $before; marker_parse_error = $markerError; capture_complete = $target.capture_complete; harness_failed = $target.harness_failed; target_process_failed = $target.target_process_failed; qualification = $qualification }
$qualificationPath = Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN.qualification-before-cleanup.json'; Write-JsonFile -Path $qualificationPath -Value $qualificationRecord
$cleanup = Remove-ExactCopiedFile -Path $destinationPath -ExpectedSha256 $destinationSha256; $apiAfter = try { (Get-FileHash -LiteralPath $apiPath -Algorithm SHA256).Hash.ToUpperInvariant() } catch { $null }; $cxlAfter = try { (Get-FileHash -LiteralPath $cxlPath -Algorithm SHA256).Hash.ToUpperInvariant() } catch { $null }
$cleanupRecord = [pscustomobject]@{ timestamp_utc = Get-UtcTimestamp; destination_file_exists_after_cleanup = $cleanup.exists_after; cleanup = $cleanup; api_sha256_before = @($preflight.artifacts | Where-Object { $_.role -eq 'amd_vendor_api_dll' })[0].sha256; api_sha256_after = $apiAfter; amd_power_profile_api_unchanged = $apiAfter -ieq $expectedApiSha256; cxl_sha256_before = @($preflight.artifacts | Where-Object { $_.role -eq 'amd_vendor_cxl_dll' })[0].sha256; cxl_sha256_after = $cxlAfter; cxl_unchanged = $cxlAfter -ieq $expectedCxlSha256 }
Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN.cleanup.json') -Value $cleanupRecord; Write-JsonFile -Path (Join-Path $OutputRoot 'STATIC-HOLD-IN-AMD-BIN-SUMMARY.json') -Value ([pscustomobject]@{ result = $qualification; evidence_root = $OutputRoot; administrator_proof = $admin; artifact_preflight = $preflight; qualification_before_cleanup = $qualificationRecord; cleanup = $cleanupRecord; hold_window_ms = $holdWindowMs; timeout_ms = $TimeoutMs; profiling_performed = $false; sampling_performed = $false; amd_api_called_from_fixture_main = $false; system_mutations_performed = $true; mutation_scope = 'exact temporary diagnostic EXE copy under D:\apps\AMDuProf\bin, followed by exact-path cleanup'; amd_runtime_executed = $true }); Write-Output "EVIDENCE_ROOT=$OutputRoot"
