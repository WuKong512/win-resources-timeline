Set-StrictMode -Version Latest

function Get-UtcTimestamp {
    [DateTime]::UtcNow.ToString('o')
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

    $parent = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($parent)) {
        [void](New-Item -ItemType Directory -Path $parent -Force)
    }
    # Windows PowerShell 5.1 otherwise reads BOM-less UTF-8 evidence using the
    # active ANSI code page, which corrupts non-ASCII wrapper_error text.
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($true))
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $json = $Value | ConvertTo-Json -Depth 30
    Write-Utf8File -Path $Path -Text $json
}

function Get-AmdCliExecutionEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$EvidenceRoot
    )

    $launchPath = Join-Path $EvidenceRoot 'AMD-CLI-LAUNCH.json'
    $completeResultPath = Join-Path $EvidenceRoot 'AMD-SERVICE-CLI-PROCESS-RESULT.json'
    $serviceRunResultPath = Join-Path $EvidenceRoot 'SERVICE-RUN-RESULT.json'
    $fallbackErrorPath = Join-Path $EvidenceRoot 'SERVICE-HARNESS-ERROR.json'
    $launchPresent = Test-Path -LiteralPath $launchPath -PathType Leaf
    $completeResultPresent = Test-Path -LiteralPath $completeResultPath -PathType Leaf

    $serviceRunResult = $null
    if (Test-Path -LiteralPath $serviceRunResultPath -PathType Leaf) {
        try {
            $serviceRunResult = Get-Content -LiteralPath $serviceRunResultPath -Raw | ConvertFrom-Json
        } catch {
            $serviceRunResult = $null
        }
    }
    $fallbackError = $null
    if (Test-Path -LiteralPath $fallbackErrorPath -PathType Leaf) {
        try {
            $fallbackError = Get-Content -LiteralPath $fallbackErrorPath -Raw | ConvertFrom-Json
        } catch {
            $fallbackError = $null
        }
    }

    $processSpawned = $launchPresent
    $targetPid = $null
    $launchEvidencePersisted = $launchPresent
    $completeResultPersisted = $completeResultPresent
    $evidenceSource = 'file-presence-fallback'

    if ($null -ne $serviceRunResult -and
        $serviceRunResult.PSObject.Properties.Name -contains 'process_spawned') {
        $processSpawned = [bool]$serviceRunResult.process_spawned
        $targetPid = $serviceRunResult.target_pid
        $launchEvidencePersisted = if ($serviceRunResult.PSObject.Properties.Name -contains 'launch_evidence_persisted') {
            [bool]$serviceRunResult.launch_evidence_persisted
        } else {
            $launchPresent
        }
        $completeResultPersisted = if ($serviceRunResult.PSObject.Properties.Name -contains 'complete_result_persisted') {
            [bool]$serviceRunResult.complete_result_persisted
        } else {
            $completeResultPresent
        }
        $evidenceSource = 'SERVICE-RUN-RESULT.json'
    } elseif ($null -ne $fallbackError -and
        (($fallbackError.PSObject.Properties.Name -contains 'process_spawned') -or
            ($fallbackError.PSObject.Properties.Name -contains 'amd_runtime_executed'))) {
        $processSpawned = if ($fallbackError.PSObject.Properties.Name -contains 'process_spawned') {
            [bool]$fallbackError.process_spawned
        } else {
            [bool]$fallbackError.amd_runtime_executed
        }
        $targetPid = if ($fallbackError.PSObject.Properties.Name -contains 'target_pid') {
            $fallbackError.target_pid
        } else {
            $null
        }
        $launchEvidencePersisted = if ($fallbackError.PSObject.Properties.Name -contains 'launch_evidence_persisted') {
            [bool]$fallbackError.launch_evidence_persisted
        } else {
            $launchPresent
        }
        $completeResultPersisted = if ($fallbackError.PSObject.Properties.Name -contains 'complete_result_persisted') {
            [bool]$fallbackError.complete_result_persisted
        } else {
            $completeResultPresent
        }
        $evidenceSource = 'SERVICE-HARNESS-ERROR.json'
    }

    if ($null -eq $targetPid -and $launchPresent) {
        try {
            $launch = Get-Content -LiteralPath $launchPath -Raw | ConvertFrom-Json
            $targetPid = $launch.target_pid
        } catch {
            $targetPid = $null
        }
    }

    # A complete execution state requires both the service's positive persistence fact and
    # the durable process-result file.  A partial file must not upgrade an incomplete run.
    $completeResultPersisted = $completeResultPersisted -and $completeResultPresent
    $executionState = if (-not $processSpawned) {
        'NOT_LAUNCHED'
    } elseif ($completeResultPersisted) {
        'LAUNCHED_COMPLETE_RESULT'
    } else {
        'LAUNCHED_INCOMPLETE_RESULT'
    }
    [pscustomobject]@{
        amd_runtime_executed = $processSpawned
        process_spawned = $processSpawned
        target_pid = $targetPid
        execution_state = $executionState
        launch_evidence_path = $launchPath
        launch_evidence_present = $launchPresent
        launch_evidence_persisted = $launchEvidencePersisted
        complete_result_path = $completeResultPath
        complete_result_present = $completeResultPresent
        complete_result_persisted = $completeResultPersisted
        service_run_result_path = $serviceRunResultPath
        fallback_error_path = $fallbackErrorPath
        evidence_source = $evidenceSource
    }
}

function Get-PeMachine {
    param([Parameter(Mandatory = $true)][string]$Path)

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x40) { throw "PE image is too small: $Path" }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($peOffset -lt 0 -or $peOffset + 6 -gt $bytes.Length) { throw "invalid PE offset: $Path" }
    if ($bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset + 1] -ne 0x45 -or
        $bytes[$peOffset + 2] -ne 0x00 -or $bytes[$peOffset + 3] -ne 0x00) {
        throw "PE signature is missing: $Path"
    }
    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $architecture = switch ($machine) {
        0x8664 { 'x64' }
        0x014c { 'x86' }
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
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Role,
        [string]$ExpectedSha256,
        [bool]$SignatureRequired = $false,
        [bool]$RequireAmdSigner = $false
    )

    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    $pe = Get-PeMachine -Path $Path
    $version = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
    $subject = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
    $issuer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Issuer } else { $null }
    $signerMatchesAmd = ($subject -match '(?i)AMD|Advanced Micro Devices') -or
        ($issuer -match '(?i)AMD|Advanced Micro Devices')
    $shaMatch = if ([string]::IsNullOrWhiteSpace($ExpectedSha256)) {
        $null
    } else {
        $hash -eq $ExpectedSha256.ToUpperInvariant()
    }
    $signaturePassed = if (-not $SignatureRequired) {
        $true
    } else {
        ($signature.Status.ToString() -eq 'Valid') -and (-not $RequireAmdSigner -or $signerMatchesAmd)
    }

    [pscustomobject]@{
        role = $Role
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
        signer_matches_amd = $signerMatchesAmd
        signature_required = $SignatureRequired
        signature_requirement_passed = $signaturePassed
        identity_passed = ($pe.architecture -eq 'x64') -and
            ($null -eq $shaMatch -or $shaMatch) -and $signaturePassed
    }
}

function Get-AdminProof {
    param([Parameter(Mandatory = $true)][string]$EvidenceRoot)

    $whoamiPath = Join-Path $env:SystemRoot 'System32\whoami.exe'
    $whoamiLines = @(& $whoamiPath /groups 2>&1 | ForEach-Object { [string]$_ })
    $whoamiExit = [int]$LASTEXITCODE
    $whoamiText = $whoamiLines -join [Environment]::NewLine
    $whoamiOutputPath = Join-Path $EvidenceRoot 'ADMIN-00-whoami-groups.txt'
    Write-Utf8File -Path $whoamiOutputPath -Text $whoamiText

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $integritySids = @(
        [regex]::Matches($whoamiText, '(?i)\bS-1-16-\d+\b') |
            ForEach-Object { $_.Value.ToUpperInvariant() } |
            Select-Object -Unique
    )
    $integrityLevels = @($integritySids | ForEach-Object { [int64]($_ -replace '^S-1-16-', '') })
    $acceptedElevatedSids = @(
        $integritySids | Where-Object { [int64]($_ -replace '^S-1-16-', '') -ge 12288 }
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
        whoami_groups_output_path = $whoamiOutputPath
        whoami_groups_output = $whoamiText
        administrator_membership = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        parsed_integrity_sids = $integritySids
        parsed_integrity_levels = $integrityLevels
        accepted_elevated_integrity_sids = $acceptedElevatedSids
        accepted_elevated_integrity_present = ($acceptedElevatedSids.Count -gt 0)
        self_elevation_performed = $false
    }
}

function Get-AmdInstallRoot {
    $keyPath = 'HKLM:\SOFTWARE\WOW6432Node\AMD\AMDProfiler'
    $valueName = 'InstallationPath'
    $record = Get-ItemProperty -LiteralPath $keyPath -Name $valueName -ErrorAction Stop
    $root = [string]$record.PSObject.Properties[$valueName].Value
    if ([string]::IsNullOrWhiteSpace($root) -or -not [System.IO.Path]::IsPathRooted($root)) {
        throw 'AMD InstallationPath is missing or not absolute'
    }
    [System.IO.Path]::GetFullPath($root).TrimEnd('\')
}

function Get-ExistingAmdCliProcesses {
    param([Parameter(Mandatory = $true)][string]$CliPath)

    $rows = @(
        Get-CimInstance -ClassName Win32_Process -Filter "Name = 'AMDuProfCLI.exe'" -ErrorAction Stop |
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
    $rows
}

function New-ProcessListEvidence {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Processes
    )

    $stableProcesses = @($Processes)
    [pscustomobject]@{
        count = [int]$stableProcesses.Count
        processes = [object[]]$stableProcesses
    }
}

function Protect-ServiceRunRoot {
    param([Parameter(Mandatory = $true)][string]$Path)

    $acl = [System.Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $inheritance = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
        [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
    $propagation = [System.Security.AccessControl.PropagationFlags]::None
    $allow = [System.Security.AccessControl.AccessControlType]::Allow
    $systemSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-18')
    $administratorsSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
    $acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new(
        $systemSid, [System.Security.AccessControl.FileSystemRights]::FullControl,
        $inheritance, $propagation, $allow))
    $acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new(
        $administratorsSid, [System.Security.AccessControl.FileSystemRights]::Modify,
        $inheritance, $propagation, $allow))
    Set-Acl -LiteralPath $Path -AclObject $acl
}

function Get-ProcessAliveById {
    param([Parameter(Mandatory = $true)][int]$ProcessId)

    try {
        $process = Get-Process -Id $ProcessId -ErrorAction Stop
        $alive = -not $process.HasExited
        $process.Dispose()
        $alive
    } catch {
        $false
    }
}

function Parse-CadenceTimestamp {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Timestamp
    )

    $match = [regex]::Match(
        $Timestamp,
        '\A(?<hour>[0-9]{1,2}):(?<minute>[0-9]{1,2}):(?<second>[0-9]{1,2}):(?<millisecond>[0-9]{1,3})\z',
        [Text.RegularExpressions.RegexOptions]::CultureInvariant
    )
    if (-not $match.Success) {
        return [pscustomobject]@{
            valid = $false
            milliseconds = $null
            error = "unsupported timestamp: $Timestamp"
        }
    }

    $invariant = [Globalization.CultureInfo]::InvariantCulture
    $hour = [int]::Parse($match.Groups['hour'].Value, $invariant)
    $minute = [int]::Parse($match.Groups['minute'].Value, $invariant)
    $second = [int]::Parse($match.Groups['second'].Value, $invariant)
    $millisecond = [int]::Parse($match.Groups['millisecond'].Value, $invariant)
    $invalidField = if ($hour -gt 23) {
        'hour'
    } elseif ($minute -gt 59) {
        'minute'
    } elseif ($second -gt 59) {
        'second'
    } elseif ($millisecond -gt 999) {
        'millisecond'
    } else {
        $null
    }
    if ($null -ne $invalidField) {
        return [pscustomobject]@{
            valid = $false
            milliseconds = $null
            error = "invalid $invalidField field in timestamp: $Timestamp"
        }
    }

    [pscustomobject]@{
        valid = $true
        milliseconds = ($hour * 3600000) + ($minute * 60000) + ($second * 1000) + $millisecond
        error = $null
    }
}

function Get-CadenceAssessment {
    param([Parameter(Mandatory = $true)]$Samples)

    $millis = @()
    foreach ($sample in @($Samples)) {
        $timestamp = [string]$sample.timestamp
        $parsedTimestamp = Parse-CadenceTimestamp -Timestamp $timestamp
        if (-not $parsedTimestamp.valid) {
            return [pscustomobject]@{
                status = 'INCONCLUSIVE'
                delta_count = 0
                deltas_ms = @()
                error = $parsedTimestamp.error
            }
        }
        $millis += [int]$parsedTimestamp.milliseconds
    }
    $deltas = @()
    $cadenceError = $null
    $midnightRollovers = 0
    $dayMillis = 86400000
    $nearMidnightStart = 23 * 3600000
    $nearMidnightEnd = 1 * 3600000
    for ($index = 1; $index -lt $millis.Count; $index++) {
        $rawDelta = $millis[$index] - $millis[$index - 1]
        $delta = $rawDelta
        if ($rawDelta -lt 0) {
            $wrappedDelta = $rawDelta + $dayMillis
            $reasonableMidnight = $midnightRollovers -eq 0 -and
                $millis[$index - 1] -ge $nearMidnightStart -and
                $millis[$index] -lt $nearMidnightEnd
            if ($reasonableMidnight) {
                $delta = $wrappedDelta
                $midnightRollovers++
            } elseif ($null -eq $cadenceError) {
                $cadenceError = 'timestamp regression was not accepted as a midnight rollover'
            }
        }
        $deltas += $delta
    }
    $status = if ($deltas.Count -gt 0 -and ($deltas | Measure-Object -Minimum).Minimum -ge 900 -and
        ($deltas | Measure-Object -Maximum).Maximum -le 1100) { 'PASS' } else { 'INCONCLUSIVE' }
    [pscustomobject]@{
        status = $status
        delta_count = $deltas.Count
        deltas_ms = $deltas
        min_ms = if ($deltas.Count -gt 0) { [int](($deltas | Measure-Object -Minimum).Minimum) } else { $null }
        max_ms = if ($deltas.Count -gt 0) { [int](($deltas | Measure-Object -Maximum).Maximum) } else { $null }
        mean_ms = if ($deltas.Count -gt 0) { [math]::Round((($deltas | Measure-Object -Average).Average), 3) } else { $null }
        error = $cadenceError
    }
}

function Test-ServiceNameAbsent {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    $null -eq (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)
}

function Test-ServiceDeleteVerified {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    $null -eq (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)
}

function Get-FixedServiceBinaryPathName {
    param(
        [Parameter(Mandatory = $true)][string]$ProbePath,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    '"{0}" --run-root "{1}"' -f $ProbePath, $RunRoot
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
