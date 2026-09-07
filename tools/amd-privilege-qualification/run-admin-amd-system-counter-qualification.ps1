#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedComparison,
    [switch]$LibraryOnly,
    [switch]$InternalTestOnlyPreMutationSentinel
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'legacy-real-gate-contract.ps1')

$ServiceName = 'ResourceTimelineAmdSystemCounterQualification'
$ServiceAccount = 'NT AUTHORITY\SYSTEM'
$ScServiceAccount = 'LocalSystem'
$ServiceSidAccount = "NT SERVICE\$ServiceName"
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = '9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3'
$QualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-counter'
$ConfigPath = Join-Path $QualificationRoot 'SYSTEM-CONFIG.json'

if ($LibraryOnly) {
    return
}

if ($InternalTestOnlyPreMutationSentinel) {
    if (-not $ExecuteAuthorizedComparison -or $env:AMD_LEGACY_OFFLINE_TEST_SENTINEL -cne 'true') {
        throw 'The I2C SYSTEM comparison offline sentinel requires -ExecuteAuthorizedComparison and AMD_LEGACY_OFFLINE_TEST_SENTINEL=true.'
    }
    Write-Host 'I2C_SYSTEM_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    return
}

if (-not $ExecuteAuthorizedComparison) {
    [ordered]@{
        qualification_only = $true
        historical_gate = 'I2C_SYSTEM_COMPARISON'
        service_name = $ServiceName
        historical_result = $I2LegacyHistoricalSystemResult
        real_gate_consumed = $I2LegacyRealGateConsumed
        real_execution_allowed = $I2cSystemComparisonAllowed
        production_account = 'UNRESOLVED'
        local_system_production_selection = 'NOT_AUTHORIZED'
        status = $I2LegacyStatus
    } | ConvertTo-Json -Depth 10
    Write-Host 'I2C_SYSTEM_PLAN_ONLY=true'
    Write-Host 'I2C_RERUN=FORBIDDEN'
    Write-Host 'SYSTEM_HISTORICAL_RESULT=AVAILABLE'
    Write-Host 'PRODUCTION_LOCAL_SYSTEM=NOT_AUTHORIZED'
    Write-Host 'No service, ACL, AMD, or evidence mutation was performed.'
    return
}

if ($I2LegacyRealGateConsumed -or -not $I2cSystemComparisonAllowed) {
    throw 'I2C_SYSTEM_COMPARISON_RERUN_FORBIDDEN: the historical I2C SYSTEM comparison gate is consumed. Use a fresh explicitly authorized experiment harness.'
}

. (Join-Path $PSScriptRoot 'sc-argument-contract.ps1')

function Assert-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'This SYSTEM comparison setup requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
    }
    if (-not [Environment]::Is64BitProcess) {
        throw 'This SYSTEM comparison setup requires x64 PowerShell.'
    }
    $identity
}

function Get-PeArchitecture {
    param([Parameter(Mandatory = $true)][string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
        throw "Artifact is not a PE image: $Path"
    }
    $offset = [BitConverter]::ToInt32($bytes, 0x3C)
    if ($offset -lt 0 -or $offset + 6 -gt $bytes.Length -or
        $bytes[$offset] -ne 0x50 -or $bytes[$offset + 1] -ne 0x45 -or
        $bytes[$offset + 2] -ne 0 -or $bytes[$offset + 3] -ne 0) {
        throw "Artifact has an invalid PE header: $Path"
    }
    switch ([BitConverter]::ToUInt16($bytes, $offset + 4)) {
        0x8664 { 'x64' }
        default { 'UNKNOWN' }
    }
}

function Invoke-Sc {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    $output = @(& $sc @Arguments 2>&1 | ForEach-Object { [string]$_ })
    if ($LASTEXITCODE -ne 0) {
        throw "sc.exe $($Arguments -join ' ') failed with exit code ${LASTEXITCODE}: $($output -join ' ')"
    }
    $output
}

function Write-Utf8Json {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    $json = $Value | ConvertTo-Json -Depth 20
    [IO.File]::WriteAllText($Path, $json, [Text.UTF8Encoding]::new($false))
}

function Set-SystemDirectoryAcl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $inherit = [Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
        [Security.AccessControl.InheritanceFlags]::ObjectInherit
    $allow = [Security.AccessControl.AccessControlType]::Allow
    foreach ($entry in @(
            @('S-1-5-18', [Security.AccessControl.FileSystemRights]::FullControl),
            @($ServiceSid, [Security.AccessControl.FileSystemRights]::FullControl),
            @('S-1-5-32-544', [Security.AccessControl.FileSystemRights]::FullControl)
        )) {
        $sid = [Security.Principal.SecurityIdentifier]::new([string]$entry[0])
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
            $sid, $entry[1], $inherit, [Security.AccessControl.PropagationFlags]::None, $allow))
    }
    Set-Acl -LiteralPath $Path -AclObject $acl
}

function Get-AmdCliPreflight {
    $installKey = 'HKLM:\SOFTWARE\WOW6432Node\AMD\AMDProfiler'
    $installRoot = [string](Get-ItemProperty -LiteralPath $installKey -Name InstallationPath -ErrorAction Stop).InstallationPath
    if ([string]::IsNullOrWhiteSpace($installRoot)) {
        throw 'AMD InstallationPath is empty.'
    }
    $cliPath = Join-Path (Join-Path $installRoot 'bin') 'AMDuProfCLI.exe'
    if (-not (Test-Path -LiteralPath $cliPath -PathType Leaf)) {
        throw "AMDuProfCLI.exe is missing from the registry-derived AMD installation: $cliPath"
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $cliPath
    $subject = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
    $issuer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Issuer } else { $null }
    $signerMatchesAmd = ($subject -match '(?i)AMD|Advanced Micro Devices') -or
        ($issuer -match '(?i)AMD|Advanced Micro Devices')
    $hash = (Get-FileHash -LiteralPath $cliPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $architecture = Get-PeArchitecture -Path $cliPath
    [pscustomobject]@{
        schema = 'amd-privilege-cli-preflight/v1'
        path = $cliPath
        installation_root = $installRoot
        sha256 = $hash
        architecture = $architecture
        signature_status = [string]$signature.Status
        signature_subject = $subject
        signature_issuer = $issuer
        signer_matches_amd = $signerMatchesAmd
        preflight_pass = ($architecture -eq 'x64' -and $signature.Status -eq 'Valid' -and $signerMatchesAmd)
    }
}

$adminIdentity = Assert-Administrator
if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
    throw "Exact release artifact is missing: $ArtifactPath"
}
$hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
if ($hash -ne $ExpectedArtifactSha256.ToUpperInvariant()) {
    throw "Release artifact SHA-256 mismatch. expected=$ExpectedArtifactSha256 actual=$hash"
}
if ((Get-PeArchitecture -Path $ArtifactPath) -ne 'x64') {
    throw 'The SYSTEM comparison artifact must be x64.'
}
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    throw "SYSTEM qualification service already exists: $ServiceName. Inspect it; do not overwrite it."
}
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) {
    throw "SYSTEM qualification config already exists: $ConfigPath. Preserve it; do not overwrite it."
}

$amdCliPreflight = Get-AmdCliPreflight
if (-not $amdCliPreflight.preflight_pass) {
    throw 'AMD CLI preflight failed; the SYSTEM comparison service was not registered.'
}

$scope = [Guid]::NewGuid().ToString('N')
$outputRoot = Join-Path $QualificationRoot $scope
$serviceCreated = $false
try {
    $binPath = '"{0}" --system-counter-service' -f $ArtifactPath
    Invoke-Sc -Arguments (New-QualificationServiceCreateArguments `
        -ServiceName $ServiceName `
        -BinPath $binPath `
        -ServiceAccount $ScServiceAccount `
        -DisplayName 'Resource Timeline AMD SYSTEM counter qualification') | Out-Null
    $serviceCreated = $true
    Invoke-Sc -Arguments @('sidtype', $ServiceName, 'unrestricted') | Out-Null

    $sidTypeOutput = @(Invoke-Sc -Arguments @('qsidtype', $ServiceName))
    if (($sidTypeOutput -join "`n") -notmatch '(?i)\bUNRESTRICTED\b') {
        throw "sc.exe qsidtype did not verify UNRESTRICTED for ${ServiceName}: $($sidTypeOutput -join ' ')"
    }

    $serviceSid = ([Security.Principal.NTAccount]::new($ServiceSidAccount)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    if ($serviceSid -notmatch '^S-1-5-80-') {
        throw "Resolved Service SID is outside the expected service SID authority: $serviceSid"
    }

    New-Item -ItemType Directory -Force -Path $QualificationRoot, $outputRoot | Out-Null
    Set-SystemDirectoryAcl -Path $QualificationRoot -ServiceSid $serviceSid
    Set-SystemDirectoryAcl -Path $outputRoot -ServiceSid $serviceSid

    Write-Utf8Json -Path (Join-Path $outputRoot 'AMD-CLI-PREFLIGHT.json') -Value $amdCliPreflight
    $config = [ordered]@{
        schema = 'amd-system-counter-config/v1'
        service_name = $ServiceName
        service_account = $ServiceAccount
        service_account_sid = 'S-1-5-18'
        service_sid = $serviceSid
        service_sid_type = 'UNRESTRICTED'
        service_sid_type_verified = $true
        scope = $scope
        output_root = $outputRoot
    }
    Write-Utf8Json -Path (Join-Path $outputRoot 'SYSTEM-CONFIG.json') -Value $config
    Write-Utf8Json -Path $ConfigPath -Value $config

    Invoke-Sc -Arguments @('start', $ServiceName) | Out-Null

    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    do {
        $contextPath = Join-Path $outputRoot 'SYSTEM-SERVICE-CONTEXT.json'
        $resultPath = Join-Path $outputRoot 'AMD-COUNTER-DISCOVERY-RESULT.json'
        $errorPath = Join-Path $outputRoot 'SYSTEM-SERVICE-HARNESS-ERROR.json'
        if ((Test-Path -LiteralPath $resultPath -PathType Leaf) -or
            (Test-Path -LiteralPath $errorPath -PathType Leaf)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)

    $errorPath = Join-Path $outputRoot 'SYSTEM-SERVICE-HARNESS-ERROR.json'
    if (Test-Path -LiteralPath $errorPath -PathType Leaf) {
        $errorText = Get-Content -LiteralPath $errorPath -Raw
        throw "SYSTEM counter qualification service failed before a valid discovery result: $errorText"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $outputRoot 'AMD-COUNTER-DISCOVERY-RESULT.json') -PathType Leaf)) {
        throw 'SYSTEM counter qualification did not produce AMD-COUNTER-DISCOVERY-RESULT.json within the bounded wait.'
    }
    if (-not (Test-Path -LiteralPath (Join-Path $outputRoot 'SYSTEM-SERVICE-CONTEXT.json') -PathType Leaf)) {
        throw 'SYSTEM counter qualification did not produce SYSTEM-SERVICE-CONTEXT.json.'
    }

    $setupResult = [ordered]@{
        schema = 'amd-system-counter-setup/v1'
        qualification_only = $true
        service_name = $ServiceName
        service_account = $ServiceAccount
        service_account_sid = 'S-1-5-18'
        service_sid = $serviceSid
        service_sid_account = $ServiceSidAccount
        service_sid_type = 'UNRESTRICTED'
        scope = $scope
        output_root = $outputRoot
        artifact_path = $ArtifactPath
        artifact_sha256 = $hash
        architecture = 'x64'
        fixed_cli_arguments = @('timechart', '--list')
        sampling = $false
        setup_and_discovery_are_coupled = $true
        counter_discovery_started_by_service = $true
        service_created = $serviceCreated
        self_elevation_performed = $false
    }
    Write-Utf8Json -Path (Join-Path $outputRoot 'SYSTEM-SETUP-RESULT.json') -Value $setupResult
    Write-Host "SYSTEM counter-discovery setup evidence retained at $outputRoot"
    Write-Host 'SETUP_AND_DISCOVERY_ARE_COUPLED=true; no power sampling was requested.'
}
catch {
    if ($serviceCreated) {
        Write-Warning "SYSTEM qualification service remains for the separate exact cleanup wrapper: $ServiceName"
    }
    throw
}
