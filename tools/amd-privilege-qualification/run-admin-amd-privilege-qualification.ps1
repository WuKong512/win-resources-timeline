#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ServiceName = 'ResourceTimelineAmdPrivilegeQualification'
$ServiceAccount = 'NT AUTHORITY\LocalService'
$ServiceSidAccount = "NT SERVICE\$ServiceName"
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = 'BD15EDE1CB886844CE6DC628926C4F54C98AB2BD6A22091A18301B2017B987AF'
$QualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege'
$ConfigPath = Join-Path $QualificationRoot 'BROKER-CONFIG.json'
. (Join-Path $PSScriptRoot 'sc-argument-contract.ps1')

function Assert-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'This setup script requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
    }
    if (-not [Environment]::Is64BitProcess) {
        throw 'This setup script requires x64 PowerShell.'
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

function Set-DirectoryQualificationAcl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$InstallingUserSid,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)][bool]$AllowInstallingUser
    )
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $inherit = [Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
        [Security.AccessControl.InheritanceFlags]::ObjectInherit
    $allow = [Security.AccessControl.AccessControlType]::Allow
    $system = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
    $admins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
    $service = [Security.Principal.SecurityIdentifier]::new($ServiceSid)
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        $system, [Security.AccessControl.FileSystemRights]::FullControl, $inherit,
        [Security.AccessControl.PropagationFlags]::None, $allow))
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        $admins, [Security.AccessControl.FileSystemRights]::FullControl, $inherit,
        [Security.AccessControl.PropagationFlags]::None, $allow))
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        $service, [Security.AccessControl.FileSystemRights]::Modify, $inherit,
        [Security.AccessControl.PropagationFlags]::None, $allow))
    if ($AllowInstallingUser) {
        $user = [Security.Principal.SecurityIdentifier]::new($InstallingUserSid)
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
            $user, [Security.AccessControl.FileSystemRights]::ReadAndExecute, $inherit,
            [Security.AccessControl.PropagationFlags]::None, $allow))
    }
    Set-Acl -LiteralPath $Path -AclObject $acl
}

function Set-ConfigAcl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$InstallingUserSid,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )
    $acl = [Security.AccessControl.FileSecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $allow = [Security.AccessControl.AccessControlType]::Allow
    foreach ($entry in @(
        @('S-1-5-18', [Security.AccessControl.FileSystemRights]::FullControl),
        @('S-1-5-32-544', [Security.AccessControl.FileSystemRights]::FullControl),
        @($ServiceSid, [Security.AccessControl.FileSystemRights]::Read),
        @($InstallingUserSid, [Security.AccessControl.FileSystemRights]::Read)
    )) {
        $sid = [Security.Principal.SecurityIdentifier]::new([string]$entry[0])
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
            $sid, $entry[1], $allow))
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
    throw 'The future broker artifact must be x64.'
}
$signature = Get-AuthenticodeSignature -LiteralPath $ArtifactPath
if ($signature.Status -notin @('Valid', 'NotSigned')) {
    throw "The broker artifact has an unusable Authenticode state: $($signature.Status)"
}
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    throw "Qualification service already exists: $ServiceName. Inspect it; do not overwrite it."
}

$installingUserSid = $adminIdentity.User.Value
$scope = [Guid]::NewGuid().ToString('N')
$outputRoot = Join-Path $QualificationRoot $scope
$serviceCreated = $false
try {
    New-Item -ItemType Directory -Force -Path $QualificationRoot, $outputRoot | Out-Null
    $amdCliPreflight = Get-AmdCliPreflight
    if (-not $amdCliPreflight.preflight_pass) {
        throw 'AMD CLI preflight failed; the service was not registered.'
    }
    Write-Utf8Json -Path (Join-Path $outputRoot 'AMD-CLI-PREFLIGHT.json') -Value $amdCliPreflight
    $config = [ordered]@{
        schema = 'amd-privilege-broker-config/v1'
        service_name = $ServiceName
        service_account = 'NT AUTHORITY\LOCAL SERVICE'
        service_account_sid = 'S-1-5-19'
        service_sid = $null
        installing_user_sid = $installingUserSid
        scope = $scope
        pipe_name = "\\.\pipe\ResourceTimeline-AmdPrivilegeQualification-$scope"
        output_root = $outputRoot
    }
    Write-Utf8Json -Path $ConfigPath -Value $config

    $binPath = '"{0}" --broker' -f $ArtifactPath
    $createArguments = New-QualificationServiceCreateArguments `
        -ServiceName $ServiceName `
        -BinPath $binPath `
        -ServiceAccount $ServiceAccount `
        -DisplayName 'Resource Timeline AMD privilege qualification broker'
    Invoke-Sc -Arguments $createArguments | Out-Null
    $serviceCreated = $true
    Invoke-Sc -Arguments @('sidtype', $ServiceName, 'unrestricted') | Out-Null
    $sidTypeOutput = (Invoke-Sc -Arguments @('qsidtype', $ServiceName)) -join [Environment]::NewLine
    if ($sidTypeOutput -notmatch '(?i)UNRESTRICTED') {
        throw "Service SID type was not verified as UNRESTRICTED: $sidTypeOutput"
    }
    $serviceSid = ([Security.Principal.NTAccount]$ServiceSidAccount).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    $config.service_sid = $serviceSid
    Write-Utf8Json -Path $ConfigPath -Value $config
    Set-DirectoryQualificationAcl -Path $QualificationRoot -InstallingUserSid $installingUserSid -ServiceSid $serviceSid -AllowInstallingUser $true
    Set-DirectoryQualificationAcl -Path $outputRoot -InstallingUserSid $installingUserSid -ServiceSid $serviceSid -AllowInstallingUser $false
    Set-ConfigAcl -Path $ConfigPath -InstallingUserSid $installingUserSid -ServiceSid $serviceSid
    Write-Utf8Json -Path (Join-Path $outputRoot 'SETUP-RESULT.json') -Value ([ordered]@{
        schema = 'amd-privilege-setup/v1'
        qualification_only = $true
        service_name = $ServiceName
        service_account = 'NT AUTHORITY\LOCAL SERVICE'
        service_account_sid = 'S-1-5-19'
        service_sid_account = $ServiceSidAccount
        service_sid = $serviceSid
        service_sid_type = 'UNRESTRICTED'
        installing_user_sid = $installingUserSid
        scope = $scope
        pipe_name = $config.pipe_name
        artifact_path = $ArtifactPath
        artifact_sha256 = $hash
        architecture = 'x64'
        broker_artifact_signature_status = [string]$signature.Status
        amd_cli_preflight_passed = $amdCliPreflight.preflight_pass
        amd_cli_path = $amdCliPreflight.path
        amd_cli_sha256 = $amdCliPreflight.sha256
        amd_runtime_started_by_setup = $false
        self_elevation_performed = $false
    })
    Invoke-Sc -Arguments @('start', $ServiceName) | Out-Null
    $readyPath = Join-Path $outputRoot 'BROKER-READY.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while (-not (Test-Path -LiteralPath $readyPath -PathType Leaf) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 250
    }
    if (-not (Test-Path -LiteralPath $readyPath -PathType Leaf)) {
        throw 'Broker did not publish BROKER-READY.json within the bounded setup wait.'
    }
    $serviceState = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($null -eq $serviceState -or $serviceState.Status -ne 'Running') {
        $observedStatus = if ($null -eq $serviceState) { 'ABSENT' } else { [string]$serviceState.Status }
        throw "BROKER-READY.json exists but qualification service is not Running (observed status: $observedStatus)."
    }
    Write-Host "Broker is ready. Keep this Administrator shell for cleanup; the standard-user client is a separate non-elevated shell."
    Write-Host "Config: $ConfigPath"
    Write-Host "Output/evidence root: $outputRoot"
    Write-Host 'No AMD runtime was started by setup.'
}
catch {
    if ($serviceCreated) {
        & (Join-Path $env:SystemRoot 'System32\sc.exe') stop $ServiceName | Out-Null
        & (Join-Path $env:SystemRoot 'System32\sc.exe') delete $ServiceName | Out-Null
    }
    throw
}
