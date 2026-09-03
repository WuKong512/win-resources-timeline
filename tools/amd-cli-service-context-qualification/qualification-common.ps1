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
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
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
    $launchPresent = Test-Path -LiteralPath $launchPath -PathType Leaf
    $completeResultPresent = Test-Path -LiteralPath $completeResultPath -PathType Leaf
    $executionState = if (-not $launchPresent) {
        'NOT_LAUNCHED'
    } elseif ($completeResultPresent) {
        'LAUNCHED_COMPLETE_RESULT'
    } else {
        'LAUNCHED_INCOMPLETE_RESULT'
    }
    [pscustomobject]@{
        amd_runtime_executed = $launchPresent
        execution_state = $executionState
        launch_evidence_path = $launchPath
        launch_evidence_present = $launchPresent
        complete_result_path = $completeResultPath
        complete_result_present = $completeResultPresent
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

function Get-CadenceAssessment {
    param([Parameter(Mandatory = $true)]$Samples)

    $millis = @()
    foreach ($sample in @($Samples)) {
        $timestamp = [string]$sample.timestamp
        if ($timestamp -notmatch '^(?<h>\d{2}):(?<m>\d{2}):(?<s>\d{2}):(?<ms>\d{3})$') {
            return [pscustomobject]@{ status = 'INCONCLUSIVE'; delta_count = 0; deltas_ms = @(); error = "unsupported timestamp: $timestamp" }
        }
        $millis += ([int]$Matches.h * 3600000) + ([int]$Matches.m * 60000) +
            ([int]$Matches.s * 1000) + [int]$Matches.ms
    }
    $deltas = @()
    for ($index = 1; $index -lt $millis.Count; $index++) {
        $delta = $millis[$index] - $millis[$index - 1]
        if ($delta -lt 0) { $delta += 86400000 }
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
        error = $null
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
