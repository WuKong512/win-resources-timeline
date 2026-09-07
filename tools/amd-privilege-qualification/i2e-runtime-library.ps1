#requires -Version 5.1

# Reusable I2E/I2F qualification helpers only.  This file has no executable
# entrypoint and intentionally declares no wrapper parameters.  Loading it may
# define pure contracts and functions, but it never creates, starts, stops, or
# deletes a service, changes LSA policy, adjusts a token, or launches AMD uProf.
. (Join-Path $PSScriptRoot 'sc-argument-contract.ps1')
. (Join-Path $PSScriptRoot 'cleanup-state-contract.ps1')
. (Join-Path $PSScriptRoot 'i2e-service-profile-contract.ps1')

function Assert-I2eAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'I2E requires an already elevated Administrator x64 PowerShell; it never self-elevates.'
    }
    if (-not [Environment]::Is64BitProcess) { throw 'I2E requires x64 PowerShell.' }
    $identity
}

function Get-I2ePeArchitecture {
    param([Parameter(Mandatory = $true)][string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
        throw ('Artifact is not a PE image: {0}' -f $Path)
    }
    $offset = [BitConverter]::ToInt32($bytes, 0x3C)
    if ($offset -lt 0 -or $offset + 6 -gt $bytes.Length -or
        $bytes[$offset] -ne 0x50 -or $bytes[$offset + 1] -ne 0x45 -or
        $bytes[$offset + 2] -ne 0 -or $bytes[$offset + 3] -ne 0) {
        throw ('Artifact has an invalid PE header: {0}' -f $Path)
    }
    if ([BitConverter]::ToUInt16($bytes, $offset + 4) -eq 0x8664) { return 'x64' }
    'UNKNOWN'
}

function Invoke-I2eSc {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    $output = @(& $sc @Arguments 2>&1 | ForEach-Object { [string]$_ })
    if ($LASTEXITCODE -ne 0) {
        throw ('sc.exe {0} failed with exit code {1}: {2}' -f
            ($Arguments -join ' '), $LASTEXITCODE, ($output -join ' '))
    }
    $output
}

function Write-I2eJson {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
}

function Read-I2eJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Get-I2eServiceSnapshot {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$ServiceName'" -ErrorAction Stop |
        Select-Object -First 1
    if ($null -eq $service) {
        return [pscustomobject]@{ present = $false; state = 'ABSENT'; process_id = 0L; start_name = $null }
    }
    [pscustomobject]@{
        present = $true
        state = [string]$service.State
        process_id = [int64]$service.ProcessId
        start_name = [string]$service.StartName
    }
}

function Get-I2eOwnedBrokerProcesses {
    param([Parameter(Mandatory = $true)][string]$ArtifactPath)
    $expectedPath = [IO.Path]::GetFullPath($ArtifactPath)
    @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expectedPath) } catch { $false }
    })
}

function Get-I2eOwnedAmdProcesses {
    param([Parameter(Mandatory = $true)][string]$ExpectedAmdCliPath)
    $expectedPath = [IO.Path]::GetFullPath($ExpectedAmdCliPath)
    @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $expectedPath) } catch { $false }
    })
}

function Set-I2eDirectoryAcl {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$ServiceSid)
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
        $rule = [Security.AccessControl.FileSystemAccessRule]::new(
            $sid, $entry[1], $inherit, [Security.AccessControl.PropagationFlags]::None, $allow)
        $acl.AddAccessRule($rule)
    }
    Set-Acl -LiteralPath $Path -AclObject $acl
}

function Get-I2eAmdCliPreflight {
    $key = 'HKLM:\SOFTWARE\WOW6432Node\AMD\AMDProfiler'
    $installRoot = [string](Get-ItemProperty -LiteralPath $key -Name InstallationPath -ErrorAction Stop).InstallationPath
    if ([string]::IsNullOrWhiteSpace($installRoot)) { throw 'AMD InstallationPath is empty.' }
    $cliPath = Join-Path (Join-Path $installRoot 'bin') 'AMDuProfCLI.exe'
    if (-not (Test-Path -LiteralPath $cliPath -PathType Leaf)) {
        throw ('AMD CLI is missing from registry-derived installation: {0}' -f $cliPath)
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $cliPath
    $subject = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
    $issuer = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Issuer } else { $null }
    $signerMatches = ($subject -match '(?i)AMD|Advanced Micro Devices') -or
        ($issuer -match '(?i)AMD|Advanced Micro Devices')
    $hash = (Get-FileHash -LiteralPath $cliPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $architecture = Get-I2ePeArchitecture -Path $cliPath
    [pscustomobject]@{
        schema = 'amd-privilege-cli-preflight/v1'
        path = $cliPath
        installation_root = $installRoot
        sha256 = $hash
        architecture = $architecture
        signature_status = [string]$signature.Status
        signature_subject = $subject
        signature_issuer = $issuer
        signer_matches_amd = $signerMatches
        preflight_pass = ($architecture -eq 'x64' -and $signature.Status -eq 'Valid' -and $signerMatches)
    }
}

function Resolve-I2eServiceSid {
    param([Parameter(Mandatory = $true)][string]$ServiceSidAccount)
    $sid = ([Security.Principal.NTAccount]::new($ServiceSidAccount)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    if ($sid -notmatch '^S-1-5-80-') { throw ('Unexpected Service SID: {0}' -f $sid) }
    $sid
}

function Assert-I2eServiceSidType {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    $output = @(Invoke-I2eSc -Arguments @('qsidtype', $ServiceName))
    if (($output -join [Environment]::NewLine) -notmatch '(?i)\bUNRESTRICTED\b') {
        throw ('Service SID type was not verified as UNRESTRICTED: {0}' -f ($output -join ' '))
    }
}

function Stop-I2eService {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    $initial = Get-I2eServiceSnapshot -ServiceName $ServiceName
    if (-not $initial.present) {
        return [pscustomobject]@{ stop_exit_code = 1062; state = 'ABSENT'; process_id = 0L; disposition = 'SERVICE_ABSENT' }
    }
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    & $sc stop $ServiceName | Out-Null
    $exitCode = [int]$LASTEXITCODE
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $current = Get-I2eServiceSnapshot -ServiceName $ServiceName
        if (-not $current.present -or ($current.state -eq 'Stopped' -and $current.process_id -eq 0)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $current = Get-I2eServiceSnapshot -ServiceName $ServiceName
    $state = if ($current.present) { $current.state } else { 'ABSENT' }
    $serviceProcessId = if ($current.present) { [int64]$current.process_id } else { 0L }
    $disposition = Resolve-QualificationStopDisposition -StopExitCode $exitCode -ServiceState $state -ServiceProcessId $serviceProcessId -ServicePresent $current.present
    if ($disposition -eq 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0') {
        throw ('I2E service did not stop safely; sc.exe exit={0}, state={1}, pid={2}' -f $exitCode, $state, $serviceProcessId)
    }
    [pscustomobject]@{ stop_exit_code = $exitCode; state = $state; process_id = $serviceProcessId; disposition = $disposition }
}

function Remove-I2eService {
    param([Parameter(Mandatory = $true)][string]$ServiceName)
    $current = Get-I2eServiceSnapshot -ServiceName $ServiceName
    if (-not $current.present) { return }
    if ($current.state -ne 'Stopped' -or $current.process_id -ne 0) { throw 'Refusing to delete a running I2E service.' }
    Invoke-I2eSc -Arguments @('delete', $ServiceName) | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if (-not (Get-I2eServiceSnapshot -ServiceName $ServiceName).present) { return }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw ('I2E service registration remains: {0}' -f $ServiceName)
}
