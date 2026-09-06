[CmdletBinding()]
param(
    [switch]$NoExecute,
    [string]$LocalServiceContextPath = 'C:\ProgramData\ResourceTimeline\qualification\amd-privilege\4b30b3d64b7e469cbce7c8080c84b7d4\SERVICE-CONTEXT.json',
    [string]$SystemContextPath = 'C:\ProgramData\ResourceTimeline\qualification\amd-system-counter\091a72e1d38341ca9eca0877b1625082\SYSTEM-SERVICE-CONTEXT.json'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-I2dPropertyValue {
    param(
        [AllowNull()][object]$Object,
        [Parameter(Mandatory)][string]$Name,
        [AllowNull()][object]$Default = $null
    )

    if ($null -eq $Object) {
        return $Default
    }

    $property = @($Object.PSObject.Properties | Where-Object { $_.Name -eq $Name } | Select-Object -First 1)
    if ($property.Count -eq 1) {
        return $property[0].Value
    }

    return $Default
}

function ConvertTo-I2dStringArray {
    param([AllowNull()][object]$Value)

    if ($null -eq $Value) {
        return @()
    }

    $items = foreach ($item in @($Value)) {
        if ($null -eq $item) {
            continue
        }

        if ($item -is [string]) {
            [string]$item
            continue
        }

        $candidate = Get-I2dPropertyValue -Object $item -Name 'name'
        if ($null -eq $candidate) {
            $candidate = Get-I2dPropertyValue -Object $item -Name 'value'
        }
        if ($null -eq $candidate) {
            $candidate = Get-I2dPropertyValue -Object $item -Name 'sid'
        }
        if ($null -ne $candidate) {
            [string]$candidate
        }
    }

    return @($items | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
}

function ConvertTo-I2dPrivilegeArray {
    param([AllowNull()][object]$Value)

    $items = foreach ($item in @($Value)) {
        if ($null -eq $item) {
            continue
        }

        if ($item -is [string]) {
            [string]$item
            continue
        }

        $candidate = Get-I2dPropertyValue -Object $item -Name 'name'
        if ($null -eq $candidate) {
            $candidate = Get-I2dPropertyValue -Object $item -Name 'privilege_name'
        }
        if ($null -eq $candidate) {
            $candidate = Get-I2dPropertyValue -Object $item -Name 'value'
        }
        if ($null -ne $candidate) {
            [string]$candidate
        }
    }

    return @($items | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
}

function ConvertTo-I2dTokenEvidence {
    param(
        [AllowNull()][object]$Context,
        [Parameter(Mandatory)][ValidateSet('LocalService', 'SYSTEM')][string]$Label
    )

    return [ordered]@{
        label = $Label
        account_sid = Get-I2dPropertyValue -Object $Context -Name 'account_sid'
        service_sid = Get-I2dPropertyValue -Object $Context -Name 'service_sid'
        session_id = Get-I2dPropertyValue -Object $Context -Name 'session_id'
        integrity_sid = Get-I2dPropertyValue -Object $Context -Name 'integrity_sid'
        token_elevated = Get-I2dPropertyValue -Object $Context -Name 'token_elevated'
        process_architecture = Get-I2dPropertyValue -Object $Context -Name 'process_architecture'
        enabled_privileges = @(ConvertTo-I2dPrivilegeArray (Get-I2dPropertyValue -Object $Context -Name 'enabled_privileges'))
        disabled_privileges = @(ConvertTo-I2dPrivilegeArray (Get-I2dPropertyValue -Object $Context -Name 'disabled_privileges'))
        token_groups_relevant_to_access = @(ConvertTo-I2dStringArray (Get-I2dPropertyValue -Object $Context -Name 'token_groups_relevant_to_access'))
    }
}

function Compare-I2dSet {
    param(
        [AllowNull()][object]$Left,
        [AllowNull()][object]$Right
    )

    $leftSet = @(ConvertTo-I2dStringArray $Left)
    $rightSet = @(ConvertTo-I2dStringArray $Right)
    return [ordered]@{
        left_only = @($leftSet | Where-Object { $rightSet -notcontains $_ })
        right_only = @($rightSet | Where-Object { $leftSet -notcontains $_ })
        common = @($leftSet | Where-Object { $rightSet -contains $_ })
    }
}

function Compare-I2dTokenEvidence {
    param(
        [Parameter(Mandatory)][object]$LocalService,
        [Parameter(Mandatory)][object]$System
    )

    $local = ConvertTo-I2dTokenEvidence -Context $LocalService -Label LocalService
    $system = ConvertTo-I2dTokenEvidence -Context $System -Label SYSTEM
    return [ordered]@{
        local_service = $local
        system = $system
        enabled_privileges = Compare-I2dSet -Left $local.enabled_privileges -Right $system.enabled_privileges
        disabled_privileges = Compare-I2dSet -Left $local.disabled_privileges -Right $system.disabled_privileges
        groups = Compare-I2dSet -Left $local.token_groups_relevant_to_access -Right $system.token_groups_relevant_to_access
        interpretation = 'Set differences are descriptive only; they do not establish causality.'
    }
}

function Read-I2dJsonEvidence {
    param([Parameter(Mandatory)][string]$Path)

    try {
        if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
            return [ordered]@{ path = $Path; status = 'UNAVAILABLE'; reason = 'ABSENT_OR_INACCESSIBLE' }
        }

        return [ordered]@{
            path = $Path
            status = 'READ'
            value = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
        }
    } catch {
        return [ordered]@{
            path = $Path
            status = 'UNAVAILABLE'
            reason = 'READ_FAILED'
            error = $_.Exception.Message
        }
    }
}

function Initialize-I2dReadOnlyLsaType {
    if ('I2dReadOnlyLsa' -as [type]) {
        return
    }

    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;

public static class I2dReadOnlyLsa
{
    [StructLayout(LayoutKind.Sequential)]
    private struct LSA_UNICODE_STRING
    {
        public ushort Length;
        public ushort MaximumLength;
        public IntPtr Buffer;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct LSA_OBJECT_ATTRIBUTES
    {
        public uint Length;
        public IntPtr RootDirectory;
        public IntPtr ObjectName;
        public uint Attributes;
        public IntPtr SecurityDescriptor;
        public IntPtr SecurityQualityOfService;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct LSA_ENUMERATION_INFORMATION
    {
        public IntPtr Sid;
    }

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern int LsaOpenPolicy(
        IntPtr SystemName,
        ref LSA_OBJECT_ATTRIBUTES ObjectAttributes,
        uint DesiredAccess,
        out IntPtr PolicyHandle);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern int LsaEnumerateAccountsWithUserRight(
        IntPtr PolicyHandle,
        ref LSA_UNICODE_STRING UserRight,
        out IntPtr EnumerationBuffer,
        out uint CountReturned);

    [DllImport("advapi32.dll")]
    private static extern int LsaFreeMemory(IntPtr Buffer);

    [DllImport("advapi32.dll")]
    private static extern int LsaClose(IntPtr ObjectHandle);

    public static string[] Enumerate(string right)
    {
        IntPtr policy = IntPtr.Zero;
        IntPtr rightBuffer = IntPtr.Zero;
        IntPtr enumeration = IntPtr.Zero;
        try
        {
            var attributes = new LSA_OBJECT_ATTRIBUTES();
            attributes.Length = (uint)Marshal.SizeOf(typeof(LSA_OBJECT_ATTRIBUTES));
            int openStatus = LsaOpenPolicy(IntPtr.Zero, ref attributes, 0x00000800, out policy);
            if (openStatus != 0)
            {
                throw new Win32Exception(openStatus, "LsaOpenPolicy failed with NTSTATUS 0x" + openStatus.ToString("X8"));
            }

            rightBuffer = Marshal.StringToHGlobalUni(right);
            var rightString = new LSA_UNICODE_STRING
            {
                Length = (ushort)(right.Length * 2),
                MaximumLength = (ushort)(right.Length * 2),
                Buffer = rightBuffer
            };

            uint count;
            int enumStatus = LsaEnumerateAccountsWithUserRight(policy, ref rightString, out enumeration, out count);
            if (enumStatus != 0)
            {
                throw new Win32Exception(enumStatus, "LsaEnumerateAccountsWithUserRight failed with NTSTATUS 0x" + enumStatus.ToString("X8"));
            }

            var result = new List<string>();
            int itemSize = Marshal.SizeOf(typeof(LSA_ENUMERATION_INFORMATION));
            for (uint index = 0; index < count; index++)
            {
                IntPtr item = IntPtr.Add(enumeration, checked((int)(index * itemSize)));
                var info = (LSA_ENUMERATION_INFORMATION)Marshal.PtrToStructure(item, typeof(LSA_ENUMERATION_INFORMATION));
                if (info.Sid != IntPtr.Zero)
                {
                    result.Add(new SecurityIdentifier(info.Sid).Value);
                }
            }

            result.Sort(StringComparer.OrdinalIgnoreCase);
            return result.ToArray();
        }
        finally
        {
            if (enumeration != IntPtr.Zero)
            {
                LsaFreeMemory(enumeration);
            }
            if (rightBuffer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(rightBuffer);
            }
            if (policy != IntPtr.Zero)
            {
                LsaClose(policy);
            }
        }
    }
}
'@
}

function Get-I2dUserRightAssignment {
    param([Parameter(Mandatory)][string]$Right)

    try {
        Initialize-I2dReadOnlyLsaType
        $principals = @([I2dReadOnlyLsa]::Enumerate($Right))
        return [ordered]@{
            right = $Right
            assigned_principals = $principals
            status = 'READ'
            local_service_has_right = ($principals -contains 'S-1-5-19')
            system_has_right = ($principals -contains 'S-1-5-18')
            administrators_has_right = ($principals -contains 'S-1-5-32-544')
            source = 'LsaEnumerateAccountsWithUserRight (read-only)'
        }
    } catch {
        return [ordered]@{
            right = $Right
            assigned_principals = @()
            status = 'UNAVAILABLE'
            reason = 'READ_ONLY_ACCESS_DENIED_OR_UNAVAILABLE'
            error = $_.Exception.Message
            local_service_has_right = $null
            system_has_right = $null
            administrators_has_right = $null
            source = 'LsaEnumerateAccountsWithUserRight (read-only)'
        }
    }
}

function Get-I2dServiceSecurityDescriptor {
    param([Parameter(Mandatory)][string]$ServiceName)

    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    try {
        $lines = @(& $sc 'sdshow' $ServiceName 2>&1)
        $exitCode = $LASTEXITCODE
        return [ordered]@{
            service_name = $ServiceName
            status = if ($exitCode -eq 0) { 'READ' } else { 'UNAVAILABLE' }
            exit_code = $exitCode
            sddl = ($lines -join [Environment]::NewLine)
            source = 'sc.exe sdshow (read-only)'
        }
    } catch {
        return [ordered]@{
            service_name = $ServiceName
            status = 'UNAVAILABLE'
            reason = 'READ_FAILED'
            error = $_.Exception.Message
            source = 'sc.exe sdshow (read-only)'
        }
    }
}

function Get-I2dServiceSidType {
    param([Parameter(Mandatory)][string]$ServiceName)

    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    try {
        $lines = @(& $sc 'qsidtype' $ServiceName 2>&1)
        $exitCode = $LASTEXITCODE
        $text = $lines -join [Environment]::NewLine
        $match = [regex]::Match($text, '(?im)SERVICE_SID_TYPE\s*:\s*(?<value>\S+)')
        return [ordered]@{
            service_name = $ServiceName
            status = if ($exitCode -eq 0 -and $match.Success) { 'READ' } else { 'UNAVAILABLE' }
            exit_code = $exitCode
            value = if ($match.Success) { $match.Groups['value'].Value } else { $null }
            source = 'sc.exe qsidtype (read-only)'
        }
    } catch {
        return [ordered]@{
            service_name = $ServiceName
            status = 'UNAVAILABLE'
            value = $null
            reason = 'READ_FAILED'
            error = $_.Exception.Message
            source = 'sc.exe qsidtype (read-only)'
        }
    }
}

function Get-I2dServiceInventory {
    $serviceNames = @('AMDPowerProfiler', 'AMDProfilerLoadService', 'AmdPpkgSvc')
    $records = foreach ($serviceName in $serviceNames) {
        $registryPath = "Registry::HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\$serviceName"
        $service = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
        $properties = $null
        try { $properties = Get-ItemProperty -LiteralPath $registryPath -ErrorAction Stop } catch { }
        [ordered]@{
            name = $serviceName
            state = if ($null -eq $service) { 'UNAVAILABLE' } else { [string]$service.Status }
            service_type = Get-I2dPropertyValue -Object $properties -Name 'Type'
            start_mode = Get-I2dPropertyValue -Object $properties -Name 'Start'
            start_account = Get-I2dPropertyValue -Object $properties -Name 'ObjectName'
            image_path = Get-I2dPropertyValue -Object $properties -Name 'ImagePath'
            dependencies = @(ConvertTo-I2dStringArray (Get-I2dPropertyValue -Object $properties -Name 'DependOnService'))
            service_sid_type = Get-I2dServiceSidType -ServiceName $serviceName
            security = Get-I2dServiceSecurityDescriptor -ServiceName $serviceName
        }
    }

    return @($records)
}

function Get-I2dFileRecord {
    param([Parameter(Mandatory)][string]$Path)

    try {
        if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
            return [ordered]@{ path = $Path; status = 'ABSENT_OR_INACCESSIBLE' }
        }

        $item = Get-Item -LiteralPath $Path -ErrorAction Stop
        $signature = Get-AuthenticodeSignature -LiteralPath $Path -ErrorAction SilentlyContinue
        $acl = Get-Acl -LiteralPath $Path -ErrorAction SilentlyContinue
        return [ordered]@{
            path = $Path
            status = 'READ'
            length = $item.Length
            sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
            file_version = $item.VersionInfo.FileVersion
            product_version = $item.VersionInfo.ProductVersion
            signature_status = if ($null -eq $signature) { 'UNAVAILABLE' } else { [string]$signature.Status }
            signer = if ($null -eq $signature.SignerCertificate) { $null } else { $signature.SignerCertificate.Subject }
            acl = if ($null -eq $acl) { $null } else { @($acl.Access | ForEach-Object { [ordered]@{ identity = [string]$_.IdentityReference; rights = [string]$_.FileSystemRights; type = [string]$_.AccessControlType; inherited = $_.IsInherited } }) }
        }
    } catch {
        return [ordered]@{ path = $Path; status = 'UNAVAILABLE'; reason = 'READ_FAILED'; error = $_.Exception.Message }
    }
}

function Get-I2dFilesystemAndRegistryForensics {
    $installRoot = 'D:\apps\AMDuProf'
    $registryRoot = 'Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\AMD\AMDProfiler'
    $paths = @(
        'D:\apps\AMDuProf\bin\AMDuProfCLI.exe',
        'D:\apps\AMDuProf\bin\AMDProfilerLoadService.exe',
        'C:\Windows\System32\drivers\AMDPowerProfiler.sys',
        'C:\Windows\System32\drivers\AMDCpuProfiler.sys'
    )

    $registry = $null
    try { $registry = Get-ItemProperty -LiteralPath $registryRoot -ErrorAction Stop } catch { }
    return [ordered]@{
        install_root = $installRoot
        registry_root = $registryRoot
        registry_status = if ($null -eq $registry) { 'UNAVAILABLE' } else { 'READ' }
        installation_path = Get-I2dPropertyValue -Object $registry -Name 'InstallationPath'
        files = @($paths | ForEach-Object { Get-I2dFileRecord -Path $_ })
        note = 'Read-only metadata and ACL inspection only; no device open, IOCTL, service, registry, or ACL mutation.'
    }
}

function Get-I2dReadOnlyReport {
    $qualificationProcesses = @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue)
    $amdCliProcesses = @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue)
    $localJson = Read-I2dJsonEvidence -Path $LocalServiceContextPath
    $systemJson = Read-I2dJsonEvidence -Path $SystemContextPath
    $localContext = if ($localJson.status -eq 'READ') { $localJson.value } else { $null }
    $systemContext = if ($systemJson.status -eq 'READ') { $systemJson.value } else { $null }
    $rights = foreach ($right in @('SeSystemProfilePrivilege', 'SeProfileSingleProcessPrivilege', 'SeDebugPrivilege', 'SeLockMemoryPrivilege', 'SeCreatePermanentPrivilege')) {
        Get-I2dUserRightAssignment -Right $right
    }

    return [ordered]@{
        schema = 'amd-privilege-i2d-readonly-forensics/v1'
        qualification_only = $true
        read_only = $true
        no_amd_runtime = $true
        no_service_mutation = $true
        no_acl_mutation = $true
        no_privilege_mutation = $true
        local_service_context = $localJson
        system_context = $systemJson
        token_differential = if ($null -ne $localContext -and $null -ne $systemContext) { Compare-I2dTokenEvidence -LocalService $localContext -System $systemContext } else { [ordered]@{ status = 'UNAVAILABLE'; reason = 'ONE_OR_BOTH_CONTEXT_FILES_UNREADABLE' } }
        user_right_assignment = @($rights)
        amd_service_driver_inventory = @(Get-I2dServiceInventory)
        filesystem_registry_forensics = Get-I2dFilesystemAndRegistryForensics
        device_interface_forensics = [ordered]@{ status = 'NOT_OBSERVED'; confidence = 'LIMITED'; note = 'No device interface or object security descriptor was identified by this bounded read-only pass; no device was opened and no IOCTL was sent.' }
        static_binary_forensics = [ordered]@{ status = 'METADATA_ONLY'; note = 'PE metadata, hashes, signatures, versions, and bounded string evidence only; binaries were not executed.' }
        current_process_safety = [ordered]@{ amd_privilege_qualification = $qualificationProcesses.Count; amd_uprof_cli = $amdCliProcesses.Count }
    }
}

if (-not $NoExecute) {
    Get-I2dReadOnlyReport | ConvertTo-Json -Depth 30
}
