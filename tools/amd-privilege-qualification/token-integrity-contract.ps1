#requires -Version 5.1

$tokenIntegrityNativeTypeName = 'ResourceTimeline.AmdPrivilege.TokenIntegrityNative'
if ($null -eq ($tokenIntegrityNativeTypeName -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace ResourceTimeline.AmdPrivilege
{
    public static class TokenIntegrityNative
    {
        public const uint TokenQuery = 0x0008;
        public const int TokenIntegrityLevel = 25;

        [DllImport("advapi32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool OpenProcessToken(
            IntPtr processHandle,
            uint desiredAccess,
            out IntPtr tokenHandle);

        [DllImport("advapi32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetTokenInformation(
            IntPtr tokenHandle,
            int tokenInformationClass,
            IntPtr tokenInformation,
            uint tokenInformationLength,
            out uint returnLength);

        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);

        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern IntPtr GetSidSubAuthority(IntPtr sid, uint subAuthority);

        [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool ConvertSidToStringSid(
            IntPtr sid,
            out IntPtr stringSid);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr LocalFree(IntPtr memory);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool CloseHandle(IntPtr handle);
    }
}
'@ | Out-Null
}

function Get-IntegrityLevelNameFromRid {
    [CmdletBinding()]
    param([AllowNull()][object]$IntegrityRid)

    if ($null -eq $IntegrityRid) {
        return 'Unknown'
    }
    try {
        $rid = [uint32]$IntegrityRid
    }
    catch {
        return 'Unknown'
    }
    switch ($rid) {
        4096 { return 'Low' }
        8192 { return 'Medium' }
        8448 { return 'MediumPlus' }
        12288 { return 'High' }
        16384 { return 'System' }
        default { return 'Unknown' }
    }
}

function Test-QualificationClientIntegrity {
    [CmdletBinding()]
    param([AllowNull()][object]$IntegrityRid)

    if ($null -eq $IntegrityRid) {
        return $false
    }
    try {
        return ([uint32]$IntegrityRid -eq 8192)
    }
    catch {
        return $false
    }
}

function Get-CurrentProcessIntegrityLevel {
    [CmdletBinding()]
    param()

    $tokenHandle = [IntPtr]::Zero
    $unmanagedBuffer = [IntPtr]::Zero
    $tokenHandleClosed = $false
    $unmanagedBufferFreed = $false
    $integritySid = $null
    $integrityRid = $null
    try {
        $processHandle = [Diagnostics.Process]::GetCurrentProcess().Handle
        if (-not [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::OpenProcessToken(
                $processHandle,
                [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::TokenQuery,
                [ref]$tokenHandle)) {
            throw "OpenProcessToken failed with Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error())."
        }

        [uint32]$requiredLength = 0
        $sizingCall = [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::GetTokenInformation(
            $tokenHandle,
            [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::TokenIntegrityLevel,
            [IntPtr]::Zero,
            0,
            [ref]$requiredLength)
        if ($requiredLength -eq 0) {
            throw "GetTokenInformation sizing failed with Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error())."
        }
        $unmanagedBuffer = [Runtime.InteropServices.Marshal]::AllocHGlobal([int]$requiredLength)
        [uint32]$returnedLength = 0
        if (-not [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::GetTokenInformation(
                $tokenHandle,
                [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::TokenIntegrityLevel,
                $unmanagedBuffer,
                $requiredLength,
                [ref]$returnedLength)) {
            throw "GetTokenInformation failed with Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error())."
        }

        $sid = [Runtime.InteropServices.Marshal]::ReadIntPtr($unmanagedBuffer)
        if ($sid -eq [IntPtr]::Zero) {
            throw 'TokenIntegrityLevel returned a null SID.'
        }
        $subAuthorityCountPointer = [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::GetSidSubAuthorityCount($sid)
        if ($subAuthorityCountPointer -eq [IntPtr]::Zero) {
            throw 'TokenIntegrityLevel SID has no sub-authority count.'
        }
        $subAuthorityCount = [Runtime.InteropServices.Marshal]::ReadByte($subAuthorityCountPointer)
        if ($subAuthorityCount -eq 0) {
            throw 'TokenIntegrityLevel SID has no sub-authorities.'
        }
        $lastSubAuthority = [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::GetSidSubAuthority(
            $sid,
            [uint32]($subAuthorityCount - 1))
        if ($lastSubAuthority -eq [IntPtr]::Zero) {
            throw 'TokenIntegrityLevel SID returned a null sub-authority.'
        }
        $integrityRid = [uint32][Runtime.InteropServices.Marshal]::ReadInt32($lastSubAuthority)

        [IntPtr]$stringSidPointer = [IntPtr]::Zero
        if (-not [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::ConvertSidToStringSid(
                $sid,
                [ref]$stringSidPointer)) {
            throw "ConvertSidToStringSid failed with Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error())."
        }
        try {
            $integritySid = [Runtime.InteropServices.Marshal]::PtrToStringUni($stringSidPointer)
        }
        finally {
            if ($stringSidPointer -ne [IntPtr]::Zero) {
                [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::LocalFree($stringSidPointer) | Out-Null
            }
        }
    }
    finally {
        if ($unmanagedBuffer -ne [IntPtr]::Zero) {
            [Runtime.InteropServices.Marshal]::FreeHGlobal($unmanagedBuffer)
            $unmanagedBufferFreed = $true
        }
        if ($tokenHandle -ne [IntPtr]::Zero) {
            $tokenHandleClosed = [ResourceTimeline.AmdPrivilege.TokenIntegrityNative]::CloseHandle($tokenHandle)
        }
    }

    [pscustomobject]@{
        integrity_sid = $integritySid
        integrity_rid = $integrityRid
        integrity_name = Get-IntegrityLevelNameFromRid -IntegrityRid $integrityRid
        token_handle_closed = [bool]$tokenHandleClosed
        unmanaged_buffer_freed = [bool]$unmanagedBufferFreed
    }
}
