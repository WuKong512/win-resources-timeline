#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$I2fServiceName = 'ResourceTimelineAmdSystemProfileEnableQualification'
$I2fServiceAccount = 'NT AUTHORITY\LOCAL SERVICE'
$I2fServiceAccountSid = 'S-1-5-19'
$I2fServiceSidAccount = "NT SERVICE\$I2fServiceName"
$I2fRequiredRight = 'SeSystemProfilePrivilege'
$I2fForbiddenPrivileges = @('SeProfileSingleProcessPrivilege', 'SeDebugPrivilege')
$I2fFixedArguments = @('timechart', '--list')
$I2fOutputSubdirectory = 'ResourceTimeline\qualification\amd-system-profile-enable'

function Get-I2fExperimentPlan {
    param([Parameter(Mandatory = $true)][string]$ArtifactSha256)

    [ordered]@{
        schema = 'amd-i2f-service-profile-experiment-plan/v1'
        qualification_only = $true
        service_name = $I2fServiceName
        service_account = $I2fServiceAccount
        service_account_sid = $I2fServiceAccountSid
        service_sid_account = $I2fServiceSidAccount
        right = $I2fRequiredRight
        fixed_cli_arguments = $I2fFixedArguments
        sampling = $false
        intentional_variable = 'SeSystemProfilePrivilege DISABLED -> ENABLED via AdjustTokenPrivileges'
        forbidden_account_wide_mutation = 'S-1-5-19'
        forbidden_group_mutation = 'S-1-5-32-544'
        forbidden_privileges = $I2fForbiddenPrivileges
        artifact_sha256 = $ArtifactSha256
        rollback = [ordered]@{
            api = 'LsaRemoveAccountRights'
            all_rights = $false
            exact_sid = 'derived dedicated Service SID'
            exact_right = $I2fRequiredRight
            token_enablement = 'dies with the qualification service process'
        }
    }
}
function Get-I2fServiceConfig {
    param(
        [Parameter(Mandatory = $true)][string]$Scope,
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)]$AmdCliPreflight
    )

    [ordered]@{
        schema = 'amd-i2f-counter-config/v1'
        qualification_only = $true
        service_name = $I2fServiceName
        service_account = $I2fServiceAccount
        service_account_sid = $I2fServiceAccountSid
        service_sid = $ServiceSid
        service_sid_type = 'UNRESTRICTED'
        service_sid_type_verified = $true
        scope = $Scope
        output_root = $OutputRoot
        expected_amd_cli_path = [string]$AmdCliPreflight.path
        expected_amd_cli_sha256 = [string]$AmdCliPreflight.sha256
        expected_amd_cli_architecture = [string]$AmdCliPreflight.architecture
        fixed_cli_arguments = $I2fFixedArguments
        sampling = $false
        counter_discovery_only = $true
        self_enable_privilege = $I2fRequiredRight
    }
}

function Compare-I2fTokenStateFixture {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After
    )

    $beforeEnabled = @($Before.enabled_privileges | Sort-Object -Unique)
    $afterEnabled = @($After.enabled_privileges | Sort-Object -Unique)
    $beforeDisabled = @($Before.disabled_privileges | Sort-Object -Unique)
    $afterDisabled = @($After.disabled_privileges | Sort-Object -Unique)
    $addedEnabled = @($afterEnabled | Where-Object { $beforeEnabled -notcontains $_ })
    $removedEnabled = @($beforeEnabled | Where-Object { $afterEnabled -notcontains $_ })
    $addedDisabled = @($afterDisabled | Where-Object { $beforeDisabled -notcontains $_ })
    $removedDisabled = @($beforeDisabled | Where-Object { $afterDisabled -notcontains $_ })
    $identityFields = @('account_sid', 'service_sid', 'session_id', 'process_architecture')
    $identityMismatches = @($identityFields | Where-Object {
            ([string]$Before.$_) -cne ([string]$After.$_)
        })
    $administratorsPresent = @($After.token_groups_relevant_to_access | Where-Object {
            ([string]$_ -split ':', 2)[0] -ieq 'S-1-5-32-544'
        }).Count -gt 0
    $forbiddenEnabled = @($I2fForbiddenPrivileges | Where-Object {
            $privilege = $_
            @($afterEnabled | Where-Object { $_ -ieq $privilege }).Count -gt 0
        })
    [pscustomobject]@{
        changed_privilege = $I2fRequiredRight
        before = 'DISABLED'
        after = 'ENABLED'
        added_enabled_privileges = $addedEnabled
        removed_enabled_privileges = $removedEnabled
        added_disabled_privileges = $addedDisabled
        removed_disabled_privileges = $removedDisabled
        identity_mismatches = $identityMismatches
        administrators_sid_present = $administratorsPresent
        forbidden_enabled_privileges = $forbiddenEnabled
        exact_one_intentional_privilege_state_change = ($addedEnabled.Count -eq 1 -and
            $addedEnabled[0] -ieq $I2fRequiredRight -and
            $removedEnabled.Count -eq 0 -and
            $addedDisabled.Count -eq 0 -and
            $removedDisabled.Count -eq 1 -and
            $removedDisabled[0] -ieq $I2fRequiredRight)
        pass = ($identityMismatches.Count -eq 0 -and
            -not $administratorsPresent -and
            $forbiddenEnabled.Count -eq 0 -and
            $addedEnabled.Count -eq 1 -and
            $addedEnabled[0] -ieq $I2fRequiredRight -and
            $removedEnabled.Count -eq 0 -and
            $addedDisabled.Count -eq 0 -and
            $removedDisabled.Count -eq 1 -and
            $removedDisabled[0] -ieq $I2fRequiredRight)
    }
}

function Test-I2fPreEnableTokenGateFixture {
    param([Parameter(Mandatory = $true)]$Context, [Parameter(Mandatory = $true)][string]$ServiceSid)

    $enabled = @($Context.enabled_privileges)
    $disabled = @($Context.disabled_privileges)
    $groups = @($Context.token_groups_relevant_to_access)
    $profileEnabled = @($enabled | Where-Object { $_ -ieq $I2fRequiredRight }).Count -gt 0
    $profileDisabled = @($disabled | Where-Object { $_ -ieq $I2fRequiredRight }).Count -gt 0
    $admins = @($groups | Where-Object { ([string]$_ -split ':', 2)[0] -ieq 'S-1-5-32-544' }).Count -gt 0
    $forbidden = @($I2fForbiddenPrivileges | Where-Object {
            $privilege = $_
            @($enabled | Where-Object { $_ -ieq $privilege }).Count -gt 0
        })
    [pscustomobject]@{
        account_sid = [string]$Context.account_sid
        service_sid = $ServiceSid
        session_id = $Context.session_id
        process_architecture = [string]$Context.process_architecture
        administrators_sid_present = $admins
        se_system_profile_privilege_present = ($profileEnabled -or $profileDisabled)
        se_system_profile_privilege_enabled = $profileEnabled
        se_system_profile_privilege_disabled = $profileDisabled
        forbidden_enabled_privileges = $forbidden
        pass = ([bool]$Context.context_valid -and
            [string]$Context.account_sid -ieq $I2fServiceAccountSid -and
            [int]$Context.session_id -eq 0 -and
            [string]$Context.process_architecture -ceq 'x64' -and
            [bool]$Context.service_sid_present -and
            -not $admins -and
            $profileDisabled -and
            -not $profileEnabled -and
            $forbidden.Count -eq 0)
    }
}

function Assert-I2fFixedArguments {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    if ($Arguments.Count -ne 2 -or $Arguments[0] -cne 'timechart' -or $Arguments[1] -cne '--list') {
        throw 'I2F accepts only the fixed non-sampling command: timechart --list.'
    }
}
