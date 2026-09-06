#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$I2eServiceName = 'ResourceTimelineAmdSystemProfileQualification'
$I2eServiceAccount = 'NT AUTHORITY\LOCAL SERVICE'
$I2eServiceAccountSid = 'S-1-5-19'
$I2eServiceSidAccount = "NT SERVICE\$I2eServiceName"
$I2eRequiredRight = 'SeSystemProfilePrivilege'
$I2eForbiddenPrivileges = @('SeProfileSingleProcessPrivilege', 'SeDebugPrivilege')
$I2eFixedArguments = @('timechart', '--list')
$I2eOutputSubdirectory = 'ResourceTimeline\qualification\amd-system-profile'

function Get-I2ePropertyValue {
    param(
        [AllowNull()][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][object]$Default = $null
    )

    if ($null -eq $Object) { return $Default }
    $property = @($Object.PSObject.Properties | Where-Object Name -eq $Name | Select-Object -First 1)
    if ($property.Count -eq 1) { return $property[0].Value }
    return $Default
}

function Get-I2eExperimentPlan {
    param([Parameter(Mandatory = $true)][string]$ArtifactSha256)

    [ordered]@{
        schema = 'amd-service-profile-experiment-plan/v1'
        qualification_only = $true
        service_name = $I2eServiceName
        service_account = $I2eServiceAccount
        service_account_sid = $I2eServiceAccountSid
        service_sid_account = $I2eServiceSidAccount
        right = $I2eRequiredRight
        fixed_cli_arguments = $I2eFixedArguments
        sampling = $false
        paired_phases = @('CONTROL', 'TREATMENT')
        intentional_variable = 'DEDICATED_SERVICE_SID_HAS_SeSystemProfilePrivilege'
        forbidden_account_wide_mutation = 'S-1-5-19'
        forbidden_group_mutation = 'S-1-5-32-544'
        artifact_sha256 = $ArtifactSha256
        rollback = [ordered]@{
            api = 'LsaRemoveAccountRights'
            all_rights = $false
            exact_sid = 'derived dedicated Service SID'
            exact_right = $I2eRequiredRight
            requires_right_added_by_experiment = $true
        }
    }
}

function Get-I2ePhaseConfig {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('CONTROL', 'TREATMENT')][string]$Phase,
        [Parameter(Mandatory = $true)][string]$Scope,
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )

    $expectedPrivilege = $Phase -eq 'TREATMENT'
    [ordered]@{
        schema = 'amd-service-profile-counter-config/v1'
        qualification_only = $true
        service_name = $I2eServiceName
        service_account = $I2eServiceAccount
        service_account_sid = $I2eServiceAccountSid
        service_sid = $ServiceSid
        service_sid_type = 'UNRESTRICTED'
        service_sid_type_verified = $true
        scope = $Scope
        output_root = $OutputRoot
        phase = $Phase
        expected_se_system_profile_privilege = $expectedPrivilege
        fixed_cli_arguments = $I2eFixedArguments
        sampling = $false
        counter_discovery_only = $true
    }
}

function Compare-I2eAmdCliPreflight {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]$Control,
        [Parameter(Mandatory = $true)]$Current
    )

    $normalize = {
        param([AllowNull()][object]$Value)
        if ($null -eq $Value) { return '' }
        ([string]$Value).Trim()
    }
    $equals = {
        param([AllowNull()][object]$Left, [AllowNull()][object]$Right)
        ([string](& $normalize $Left)).Equals([string](& $normalize $Right), [StringComparison]::OrdinalIgnoreCase)
    }
    $controlPreflightPass = [bool](Get-I2ePropertyValue -Object $Control -Name 'preflight_pass')
    $currentPreflightPass = [bool](Get-I2ePropertyValue -Object $Current -Name 'preflight_pass')
    $controlSignatureStatus = [string](Get-I2ePropertyValue -Object $Control -Name 'signature_status')
    $currentSignatureStatus = [string](Get-I2ePropertyValue -Object $Current -Name 'signature_status')
    $controlSignerMatches = [bool](Get-I2ePropertyValue -Object $Control -Name 'signer_matches_amd')
    $currentSignerMatches = [bool](Get-I2ePropertyValue -Object $Current -Name 'signer_matches_amd')
    $comparison = [ordered]@{
        control_preflight_pass = $controlPreflightPass
        current_preflight_pass = $currentPreflightPass
        path_match = & $equals (Get-I2ePropertyValue -Object $Control -Name 'path') (Get-I2ePropertyValue -Object $Current -Name 'path')
        installation_root_match = & $equals (Get-I2ePropertyValue -Object $Control -Name 'installation_root') (Get-I2ePropertyValue -Object $Current -Name 'installation_root')
        sha256_match = & $equals (Get-I2ePropertyValue -Object $Control -Name 'sha256') (Get-I2ePropertyValue -Object $Current -Name 'sha256')
        architecture_match = & $equals (Get-I2ePropertyValue -Object $Control -Name 'architecture') (Get-I2ePropertyValue -Object $Current -Name 'architecture')
        signature_status_match = & $equals $controlSignatureStatus $currentSignatureStatus
        signature_valid = $controlSignatureStatus -ceq 'Valid' -and $currentSignatureStatus -ceq 'Valid'
        amd_signer_valid = $controlSignerMatches -and $currentSignerMatches
        signature_subject_match = & $equals (Get-I2ePropertyValue -Object $Control -Name 'signature_subject') (Get-I2ePropertyValue -Object $Current -Name 'signature_subject')
        signature_issuer_match = & $equals (Get-I2ePropertyValue -Object $Control -Name 'signature_issuer') (Get-I2ePropertyValue -Object $Current -Name 'signature_issuer')
    }
    $differingFields = @($comparison.Keys | Where-Object { -not [bool]$comparison[$_] })
    [pscustomobject]@{
        pass = ($differingFields.Count -eq 0)
        control_identity = $Control
        current_identity = $Current
        comparison = $comparison
        differing_fields = $differingFields
    }
}

function Get-I2eDiagnosticException {
    param([AllowNull()][object]$Exception)

    $current = $Exception
    for ($depth = 0; $depth -lt 8 -and $null -ne $current; $depth++) {
        $properties = @($current.PSObject.Properties)
        if (@($properties | Where-Object Name -eq 'NtStatusHex').Count -gt 0 -or
            @($properties | Where-Object Name -eq 'Win32Error').Count -gt 0) {
            return $current
        }
        $current = if (@($properties | Where-Object Name -eq 'InnerException').Count -gt 0) {
            $current.InnerException
        } else {
            $null
        }
    }
    return $Exception
}

function Initialize-I2eLsaMutationType {
    if ('I2eLsaMutation' -as [type]) {
        return
    }

    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Security.Principal;

public sealed class I2eLsaException : Exception
{
    public int NtStatus { get; private set; }
    public uint Win32Error { get; private set; }
    public string NtStatusHex { get { return "0x" + unchecked((uint)NtStatus).ToString("X8"); } }

    public I2eLsaException(string operation, int ntStatus, uint win32Error)
        : base(operation + " failed with NTSTATUS 0x" + unchecked((uint)ntStatus).ToString("X8") +
               " and Win32 error " + win32Error.ToString())
    {
        NtStatus = ntStatus;
        Win32Error = win32Error;
    }
}

public sealed class I2eAccountRightsResult
{
    public string[] Rights { get; private set; }
    public string AccountObjectState { get; private set; }

    public I2eAccountRightsResult(string[] rights, string accountObjectState)
    {
        Rights = rights ?? new string[0];
        AccountObjectState = accountObjectState;
    }
}

public static class I2eLsaMutation
{
    private const uint POLICY_VIEW_LOCAL_INFORMATION = 0x00000001;
    private const uint POLICY_CREATE_ACCOUNT = 0x00000010;
    private const uint POLICY_LOOKUP_NAMES = 0x00000800;
    private const uint READ_POLICY_ACCESS = 0x00000801;
    private const uint ADD_POLICY_ACCESS = 0x00000810;
    private const uint REMOVE_POLICY_ACCESS = 0x00000800;
    private const int STATUS_OBJECT_NAME_NOT_FOUND = unchecked((int)0xC0000034);
    private const int STATUS_NO_MORE_ENTRIES = unchecked((int)0x8000001A);

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

    [DllImport("advapi32.dll")]
    private static extern int LsaOpenPolicy(
        IntPtr SystemName,
        ref LSA_OBJECT_ATTRIBUTES ObjectAttributes,
        uint DesiredAccess,
        out IntPtr PolicyHandle);

    [DllImport("advapi32.dll")]
    private static extern uint LsaNtStatusToWinError(int Status);

    [DllImport("advapi32.dll")]
    private static extern int LsaEnumerateAccountRights(
        IntPtr PolicyHandle,
        IntPtr AccountSid,
        out IntPtr UserRights,
        out uint CountOfRights);

    [DllImport("advapi32.dll")]
    private static extern int LsaEnumerateAccountsWithUserRight(
        IntPtr PolicyHandle,
        ref LSA_UNICODE_STRING UserRight,
        out IntPtr EnumerationBuffer,
        out uint CountReturned);

    [DllImport("advapi32.dll")]
    private static extern int LsaAddAccountRights(
        IntPtr PolicyHandle,
        IntPtr AccountSid,
        ref LSA_UNICODE_STRING UserRights,
        uint CountOfRights);

    [DllImport("advapi32.dll")]
    private static extern int LsaRemoveAccountRights(
        IntPtr PolicyHandle,
        IntPtr AccountSid,
        bool AllRights,
        ref LSA_UNICODE_STRING UserRights,
        uint CountOfRights);

    [DllImport("advapi32.dll")]
    private static extern int LsaFreeMemory(IntPtr Buffer);

    [DllImport("advapi32.dll")]
    private static extern int LsaClose(IntPtr ObjectHandle);

    private static void ThrowIfFailed(string operation, int status)
    {
        if (status != 0)
        {
            throw new I2eLsaException(operation, status, LsaNtStatusToWinError(status));
        }
    }

    private static IntPtr OpenPolicy(uint desiredAccess)
    {
        IntPtr policy;
        var attributes = new LSA_OBJECT_ATTRIBUTES();
        attributes.Length = (uint)Marshal.SizeOf(typeof(LSA_OBJECT_ATTRIBUTES));
        ThrowIfFailed("LsaOpenPolicy", LsaOpenPolicy(
            IntPtr.Zero, ref attributes, desiredAccess, out policy));
        return policy;
    }

    private static IntPtr AllocateSid(string sidString)
    {
        var sid = new SecurityIdentifier(sidString);
        var bytes = new byte[sid.BinaryLength];
        sid.GetBinaryForm(bytes, 0);
        var buffer = Marshal.AllocHGlobal(bytes.Length);
        Marshal.Copy(bytes, 0, buffer, bytes.Length);
        return buffer;
    }

    private static IntPtr AllocateRight(string right, out LSA_UNICODE_STRING value)
    {
        var buffer = Marshal.StringToHGlobalUni(right);
        value = new LSA_UNICODE_STRING {
            Length = checked((ushort)(right.Length * 2)),
            MaximumLength = checked((ushort)(right.Length * 2)),
            Buffer = buffer
        };
        return buffer;
    }

    private static string ReadUnicodeString(LSA_UNICODE_STRING value)
    {
        if (value.Buffer == IntPtr.Zero || value.Length == 0)
        {
            return String.Empty;
        }
        return Marshal.PtrToStringUni(value.Buffer, value.Length / 2);
    }

    public static I2eAccountRightsResult EnumerateAccountRightsDetailed(string sidString)
    {
        IntPtr policy = IntPtr.Zero;
        IntPtr sid = IntPtr.Zero;
        IntPtr rights = IntPtr.Zero;
        try
        {
            policy = OpenPolicy(READ_POLICY_ACCESS);
            sid = AllocateSid(sidString);
            uint count;
            var status = LsaEnumerateAccountRights(policy, sid, out rights, out count);
            if (status == STATUS_OBJECT_NAME_NOT_FOUND)
            {
                return new I2eAccountRightsResult(
                    new string[0], "ABSENT");
            }
            if (status == STATUS_NO_MORE_ENTRIES)
            {
                return new I2eAccountRightsResult(
                    new string[0], "UNKNOWN");
            }
            ThrowIfFailed("LsaEnumerateAccountRights", status);
            var result = new List<string>();
            var itemSize = Marshal.SizeOf(typeof(LSA_UNICODE_STRING));
            for (uint index = 0; index < count; index++)
            {
                var item = IntPtr.Add(rights, checked((int)(index * itemSize)));
                var value = (LSA_UNICODE_STRING)Marshal.PtrToStructure(
                    item, typeof(LSA_UNICODE_STRING));
                result.Add(ReadUnicodeString(value));
            }
            result.Sort(StringComparer.OrdinalIgnoreCase);
            return new I2eAccountRightsResult(result.ToArray(), "PRESENT");
        }
        finally
        {
            if (rights != IntPtr.Zero) LsaFreeMemory(rights);
            if (sid != IntPtr.Zero) Marshal.FreeHGlobal(sid);
            if (policy != IntPtr.Zero) LsaClose(policy);
        }
    }

    public static string[] EnumerateAccountRights(string sidString)
    {
        return EnumerateAccountRightsDetailed(sidString).Rights;
    }

    public static string[] EnumerateAccountsWithUserRight(string right)
    {
        IntPtr policy = IntPtr.Zero;
        IntPtr rightBuffer = IntPtr.Zero;
        IntPtr enumeration = IntPtr.Zero;
        try
        {
            policy = OpenPolicy(READ_POLICY_ACCESS);
            LSA_UNICODE_STRING rightValue;
            rightBuffer = AllocateRight(right, out rightValue);
            uint count;
            var status = LsaEnumerateAccountsWithUserRight(
                policy, ref rightValue, out enumeration, out count);
            if (status == STATUS_OBJECT_NAME_NOT_FOUND || status == STATUS_NO_MORE_ENTRIES)
            {
                return new string[0];
            }
            ThrowIfFailed("LsaEnumerateAccountsWithUserRight", status);
            var result = new List<string>();
            var itemSize = IntPtr.Size;
            for (uint index = 0; index < count; index++)
            {
                var item = IntPtr.Add(enumeration, checked((int)(index * itemSize)));
                var sid = Marshal.ReadIntPtr(item);
                result.Add(new SecurityIdentifier(sid).Value);
            }
            result.Sort(StringComparer.OrdinalIgnoreCase);
            return result.ToArray();
        }
        finally
        {
            if (enumeration != IntPtr.Zero) LsaFreeMemory(enumeration);
            if (rightBuffer != IntPtr.Zero) Marshal.FreeHGlobal(rightBuffer);
            if (policy != IntPtr.Zero) LsaClose(policy);
        }
    }

    public static void AddExactRight(string sidString, string right)
    {
        IntPtr policy = IntPtr.Zero;
        IntPtr sid = IntPtr.Zero;
        IntPtr rightBuffer = IntPtr.Zero;
        try
        {
            policy = OpenPolicy(ADD_POLICY_ACCESS);
            sid = AllocateSid(sidString);
            LSA_UNICODE_STRING rightValue;
            rightBuffer = AllocateRight(right, out rightValue);
            ThrowIfFailed("LsaAddAccountRights", LsaAddAccountRights(
                policy, sid, ref rightValue, 1));
        }
        finally
        {
            if (rightBuffer != IntPtr.Zero) Marshal.FreeHGlobal(rightBuffer);
            if (sid != IntPtr.Zero) Marshal.FreeHGlobal(sid);
            if (policy != IntPtr.Zero) LsaClose(policy);
        }
    }

    public static void RemoveExactRight(string sidString, string right)
    {
        IntPtr policy = IntPtr.Zero;
        IntPtr sid = IntPtr.Zero;
        IntPtr rightBuffer = IntPtr.Zero;
        try
        {
            policy = OpenPolicy(REMOVE_POLICY_ACCESS);
            sid = AllocateSid(sidString);
            LSA_UNICODE_STRING rightValue;
            rightBuffer = AllocateRight(right, out rightValue);
            ThrowIfFailed("LsaRemoveAccountRights", LsaRemoveAccountRights(
                policy, sid, false, ref rightValue, 1));
        }
        finally
        {
            if (rightBuffer != IntPtr.Zero) Marshal.FreeHGlobal(rightBuffer);
            if (sid != IntPtr.Zero) Marshal.FreeHGlobal(sid);
            if (policy != IntPtr.Zero) LsaClose(policy);
        }
    }
}
'@
}

function Get-I2eDirectAccountRightsSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string]$Sid
    )

    try {
        Initialize-I2eLsaMutationType
        $rights = [I2eLsaMutation]::EnumerateAccountRightsDetailed($Sid)
        [ordered]@{
            label = $Label
            account_sid = $Sid
            direct_rights = @($rights.Rights)
            account_object_state = [string]$rights.AccountObjectState
            status = 'READ'
            source = 'LsaEnumerateAccountRights (read-only)'
        }
    } catch {
        $exception = Get-I2eDiagnosticException -Exception $_.Exception
        [ordered]@{
            label = $Label
            account_sid = $Sid
            direct_rights = @()
            account_object_state = 'UNKNOWN'
            status = 'UNAVAILABLE'
            error = $exception.Message
            ntstatus_hex = Get-I2ePropertyValue -Object $exception -Name 'NtStatusHex'
            win32_error = Get-I2ePropertyValue -Object $exception -Name 'Win32Error'
            source = 'LsaEnumerateAccountRights (read-only)'
        }
    }
}

function Get-I2eUserRightAssignmentSnapshot {
    param([Parameter(Mandatory = $true)][string]$Right)

    if ($Right -cne $I2eRequiredRight) {
        throw "I2E assignment query is fixed to $I2eRequiredRight."
    }
    try {
        Initialize-I2eLsaMutationType
        [ordered]@{
            right = $Right
            assigned_principals = @([I2eLsaMutation]::EnumerateAccountsWithUserRight($Right))
            status = 'READ'
            source = 'LsaEnumerateAccountsWithUserRight (read-only)'
        }
    } catch {
        $exception = Get-I2eDiagnosticException -Exception $_.Exception
        [ordered]@{
            right = $Right
            assigned_principals = @()
            status = 'UNAVAILABLE'
            error = $exception.Message
            ntstatus_hex = Get-I2ePropertyValue -Object $exception -Name 'NtStatusHex'
            win32_error = Get-I2ePropertyValue -Object $exception -Name 'Win32Error'
            source = 'LsaEnumerateAccountsWithUserRight (read-only)'
        }
    }
}

function Add-I2eExactServiceProfileRight {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)

    if ($ServiceSid -notmatch '^S-1-5-80-') {
        throw "Refusing to mutate a non-Service SID: $ServiceSid"
    }
    Initialize-I2eLsaMutationType
    [I2eLsaMutation]::AddExactRight($ServiceSid, $I2eRequiredRight)
}

function Remove-I2eExactServiceProfileRight {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)

    if ($ServiceSid -notmatch '^S-1-5-80-') {
        throw "Refusing to mutate a non-Service SID: $ServiceSid"
    }
    Initialize-I2eLsaMutationType
    [I2eLsaMutation]::RemoveExactRight($ServiceSid, $I2eRequiredRight)
}

function Compare-I2eTokenDelta {
    param(
        [Parameter(Mandatory = $true)]$ControlContext,
        [Parameter(Mandatory = $true)]$TreatmentContext
    )

    $controlEnabled = @($ControlContext.enabled_privileges | Sort-Object -Unique)
    $treatmentEnabled = @($TreatmentContext.enabled_privileges | Sort-Object -Unique)
    $addedEnabled = @($treatmentEnabled | Where-Object { $controlEnabled -notcontains $_ })
    $removedEnabled = @($controlEnabled | Where-Object { $treatmentEnabled -notcontains $_ })
    $unexpectedAdded = @($addedEnabled | Where-Object { $_ -cne $I2eRequiredRight })
    $controlDisabled = @($ControlContext.disabled_privileges | Sort-Object -Unique)
    $treatmentDisabled = @($TreatmentContext.disabled_privileges | Sort-Object -Unique)
    $addedDisabled = @($treatmentDisabled | Where-Object { $controlDisabled -notcontains $_ })
    $removedDisabled = @($controlDisabled | Where-Object { $treatmentDisabled -notcontains $_ })
    $controlGroups = @($ControlContext.token_groups_relevant_to_access | Sort-Object -Unique)
    $treatmentGroups = @($TreatmentContext.token_groups_relevant_to_access | Sort-Object -Unique)
    $addedGroups = @($treatmentGroups | Where-Object { $controlGroups -notcontains $_ })
    $removedGroups = @($controlGroups | Where-Object { $treatmentGroups -notcontains $_ })
    $identityFields = @('account_sid', 'service_sid', 'session_id', 'process_architecture')
    $identityMismatches = @($identityFields | Where-Object {
        (Get-I2ePropertyValue -Object $ControlContext -Name $_) -ne
        (Get-I2ePropertyValue -Object $TreatmentContext -Name $_)
    })
    [ordered]@{
        control_enabled_privileges = $controlEnabled
        treatment_enabled_privileges = $treatmentEnabled
        added_enabled_privileges = $addedEnabled
        removed_enabled_privileges = $removedEnabled
        unexpected_added_enabled_privileges = $unexpectedAdded
        control_disabled_privileges = $controlDisabled
        treatment_disabled_privileges = $treatmentDisabled
        added_disabled_privileges = $addedDisabled
        removed_disabled_privileges = $removedDisabled
        added_relevant_groups = $addedGroups
        removed_relevant_groups = $removedGroups
        identity_mismatches = $identityMismatches
        expected_delta = @($I2eRequiredRight)
        pass = ($addedEnabled.Count -eq 1 -and
            $addedEnabled[0] -ceq $I2eRequiredRight -and
            $removedEnabled.Count -eq 0 -and
            $unexpectedAdded.Count -eq 0 -and
            $addedDisabled.Count -eq 0 -and
            $removedDisabled.Count -eq 0 -and
            $addedGroups.Count -eq 0 -and
            $removedGroups.Count -eq 0 -and
            $identityMismatches.Count -eq 0)
    }
}
