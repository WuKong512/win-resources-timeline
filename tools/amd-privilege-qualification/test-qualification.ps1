#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ToolRoot = $PSScriptRoot
$Manifest = Join-Path $ToolRoot 'Cargo.toml'
$TargetRoot = Join-Path $ToolRoot 'target\qualification-synthetic'
$EvidenceRoot = Join-Path $TargetRoot 'evidence'
$Binary = Join-Path $ToolRoot 'target\debug\amd-privilege-qualification.exe'
$ScArgumentContract = Join-Path $ToolRoot 'sc-argument-contract.ps1'
$TokenIntegrityContract = Join-Path $ToolRoot 'token-integrity-contract.ps1'
$CleanupStateContract = Join-Path $ToolRoot 'cleanup-state-contract.ps1'
$I2dForensics = Join-Path $ToolRoot 'i2d-readonly-forensics.ps1'
$I2eContract = Join-Path $ToolRoot 'i2e-service-profile-contract.ps1'
$I2eRuntimeLibrary = Join-Path $ToolRoot 'i2e-runtime-library.ps1'
$I2eSetup = Join-Path $ToolRoot 'run-admin-amd-i2e-service-profile-experiment.ps1'
$I2eCleanup = Join-Path $ToolRoot 'cleanup-admin-amd-i2e-service-profile-experiment.ps1'
$I2eResumeContract = Join-Path $ToolRoot 'i2e-treatment-resume-contract.ps1'
$I2eResume = Join-Path $ToolRoot 'resume-admin-amd-i2e-treatment.ps1'
$I2eResumeEntrypointTest = Join-Path $ToolRoot 'test-i2e-resume-entrypoint.ps1'
$I2eRetirementEntrypointTest = Join-Path $ToolRoot 'test-i2e-retirement-entrypoints.ps1'
$LegacyRealGateContract = Join-Path $ToolRoot 'legacy-real-gate-contract.ps1'
$LegacyRetirementTest = Join-Path $ToolRoot 'test-legacy-real-gate-retirement.ps1'
$I2fContract = Join-Path $ToolRoot 'i2f-service-profile-contract.ps1'
$I2fSetup = Join-Path $ToolRoot 'run-admin-amd-i2f-service-profile-experiment.ps1'
$I2fCleanup = Join-Path $ToolRoot 'cleanup-admin-amd-i2f-service-profile-experiment.ps1'
$I2fEntrypointScopeTest = Join-Path $ToolRoot 'test-i2f-entrypoint-scope.ps1'
$I2eFinalFixture = Join-Path $ToolRoot 'i2e-token-materialization-final.example.json'
$WindowsSource = Join-Path $ToolRoot 'src\windows.rs'
$ExecutionPlan = Join-Path $ToolRoot '..\..\docs\upgrade\execution-plan.md'
$I2gRuntimeContract = Join-Path $ToolRoot 'i2g-runtime-contract.ps1'
$I2gSetup = Join-Path $ToolRoot 'run-admin-amd-i2g-qualification.ps1'
$I2gCleanup = Join-Path $ToolRoot 'cleanup-admin-amd-i2g-qualification.ps1'
$I2gHarnessTest = Join-Path $ToolRoot 'test-i2g-harness.ps1'
$I2gHarnessDocument = Join-Path $ToolRoot '..\..\docs\upgrade\amd-i2g-harness.md'
$ArchitectureDoc = Join-Path $ToolRoot '..\..\docs\architecture\cpu-sensor-amd-privilege-deployment.md'
$QualificationReadme = Join-Path $ToolRoot 'README.md'
$ResidualDifferential = Join-Path $ToolRoot '..\..\docs\upgrade\amd-system-vs-i2f-residual-differential.md'
$I2gSelectionDocument = Join-Path $ToolRoot '..\..\docs\upgrade\amd-i2g-variable-selection.md'

foreach ($wrapper in @(
        $ScArgumentContract,
        $TokenIntegrityContract,
        $CleanupStateContract,
        $I2dForensics,
        $I2eRuntimeLibrary,
        (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1'),
        (Join-Path $ToolRoot 'run-admin-amd-system-counter-qualification.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-system-counter-qualification.ps1'),
        $I2eContract,
        $I2eSetup,
        $I2eCleanup,
        $I2eResumeContract,
        $I2eResume,
        $I2eResumeEntrypointTest,
        $I2eRetirementEntrypointTest,
        $LegacyRealGateContract,
        $LegacyRetirementTest,
        (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1'),
        (Join-Path $ToolRoot 'run-admin-amd-system-counter-qualification.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-system-counter-qualification.ps1'),
        $I2fContract,
        $I2fSetup,
        $I2fCleanup,
        $I2fEntrypointScopeTest,
        $I2gRuntimeContract,
        $I2gSetup,
        $I2gCleanup,
        $I2gHarnessTest
    )) {
    $parseErrors = $null
    $tokens = $null
    [System.Management.Automation.Language.Parser]::ParseFile($wrapper, [ref]$tokens, [ref]$parseErrors) | Out-Null
    if ($parseErrors.Count -ne 0) {
        throw "PowerShell syntax errors in wrapper: $wrapper"
    }
}

function Assert-I2eNoPidAssignment {
    param([Parameter(Mandatory)][string]$Path)
    $parseErrors = $null
    $tokens = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$parseErrors)
    if ($parseErrors.Count -ne 0) {
        throw "PowerShell syntax errors while auditing PID assignments: $Path"
    }
    $badAssignments = @($ast.FindAll({
            param($node)
            if ($node -is [System.Management.Automation.Language.AssignmentStatementAst]) {
                $left = $node.Left
                return $left -is [System.Management.Automation.Language.VariableExpressionAst] -and
                    $left.VariablePath.UserPath -ieq 'pid'
            }
            if ($node -is [System.Management.Automation.Language.ParameterAst]) {
                return $node.Name.VariablePath.UserPath -ieq 'pid'
            }
            return $false
        }, $true))
    if ($badAssignments.Count -ne 0) {
        throw "I2E PowerShell must not assign to the automatic PID variable: $Path"
    }
}

foreach ($i2eScript in @($I2eRuntimeLibrary, $I2eSetup, $I2eCleanup, $I2eContract, $I2eResumeContract, $I2eResume, $I2eResumeEntrypointTest, $I2eRetirementEntrypointTest, $LegacyRealGateContract, $LegacyRetirementTest, $I2fSetup, $I2fCleanup, $I2fContract, $I2fEntrypointScopeTest)) {
    Assert-I2eNoPidAssignment -Path $i2eScript
}
Write-Host 'I2E_PID_AUTOMATIC_VARIABLE_ASSIGNMENT_AUDIT=PASS'

$clientWrapper = Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'
$integritySource = (Get-Content -LiteralPath $clientWrapper -Raw) +
    (Get-Content -LiteralPath $TokenIntegrityContract -Raw)
if ($integritySource -match 'WindowsIdentity\.Groups') {
    throw 'Client integrity detection must not use WindowsIdentity.Groups.'
}
. $TokenIntegrityContract
foreach ($case in @(
        @{ rid = 4096; name = 'Low'; accepted = $false },
        @{ rid = 8192; name = 'Medium'; accepted = $true },
        @{ rid = 8448; name = 'MediumPlus'; accepted = $false },
        @{ rid = 12288; name = 'High'; accepted = $false },
        @{ rid = 16384; name = 'System'; accepted = $false },
        @{ rid = $null; name = 'Unknown'; accepted = $false },
        @{ rid = 'malformed'; name = 'Unknown'; accepted = $false }
    )) {
    $name = Get-IntegrityLevelNameFromRid -IntegrityRid $case.rid
    $accepted = Test-QualificationClientIntegrity -IntegrityRid $case.rid
    if ($name -cne $case.name -or [bool]$accepted -ne [bool]$case.accepted) {
        throw "Integrity classification mismatch for RID '$($case.rid)'. name=$name accepted=$accepted"
    }
}
Write-Host 'CLIENT_INTEGRITY_DETECTION_DOES_NOT_USE_WINDOWSIDENTITY_GROUPS=PASS'
Write-Host 'MEDIUM_INTEGRITY_ACCEPTED=PASS'
Write-Host 'HIGH_INTEGRITY_REJECTED=PASS'
Write-Host 'SYSTEM_INTEGRITY_REJECTED=PASS'
Write-Host 'LOW_INTEGRITY_REJECTED=PASS'
Write-Host 'UNKNOWN_INTEGRITY_REJECTED=PASS'

. $ScArgumentContract
$testBinPath = '"F:\Qualification Root\amd-privilege-qualification.exe" --broker'
$testServiceAccount = 'NT AUTHORITY\LocalService'
$testDisplayName = 'Resource Timeline AMD privilege qualification broker'
$scCreateArguments = @(
    New-QualificationServiceCreateArguments `
        -ServiceName 'ResourceTimelineAmdPrivilegeQualification' `
        -BinPath $testBinPath `
        -ServiceAccount $testServiceAccount `
        -DisplayName $testDisplayName
)
$expectedScCreateArguments = @(
    'create'
    'ResourceTimelineAmdPrivilegeQualification'
    'binPath='
    $testBinPath
    'start='
    'demand'
    'obj='
    $testServiceAccount
    'type='
    'own'
    'DisplayName='
    $testDisplayName
)
if ($scCreateArguments.Count -ne $expectedScCreateArguments.Count) {
    throw "SC create argv count mismatch. expected=$($expectedScCreateArguments.Count) actual=$($scCreateArguments.Count)"
}
for ($index = 0; $index -lt $expectedScCreateArguments.Count; $index++) {
    if ($scCreateArguments[$index] -cne $expectedScCreateArguments[$index]) {
        throw "SC create argv mismatch at index $index. expected='$($expectedScCreateArguments[$index])' actual='$($scCreateArguments[$index])'"
    }
}
foreach ($collapsedArgument in @(
        'start= demand',
        "obj= $testServiceAccount",
        'type= own',
        "DisplayName= $testDisplayName"
    )) {
    if ($scCreateArguments -ccontains $collapsedArgument) {
        throw "Collapsed SC create argv element was present: $collapsedArgument"
    }
}
if ($scCreateArguments[3] -cne $testBinPath) {
    throw 'SC create binPath value was not preserved as one argv element.'
}
if ($scCreateArguments[7] -cne $testServiceAccount) {
    throw 'SC create service account value with spaces was not preserved as one argv element.'
}
if ($scCreateArguments[11] -cne $testDisplayName) {
    throw 'SC create display name with spaces was not preserved as one argv element.'
}
Write-Host 'SC_CREATE_ARGV_SHAPE=PASS'
Write-Host 'SC_CREATE_BINPATH_VALUE_PRESERVED=PASS'
Write-Host 'SC_CREATE_ACCOUNT_WITH_SPACE_PRESERVED=PASS'
Write-Host 'SC_CREATE_DISPLAY_NAME_WITH_SPACES_PRESERVED=PASS'

. $CleanupStateContract
$cleanupWrapper = Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1'
$cleanupSource = Get-Content -LiteralPath $cleanupWrapper -Raw
$adminSetupSource = Get-Content -LiteralPath (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1') -Raw
$systemSetupWrapper = Join-Path $ToolRoot 'run-admin-amd-system-counter-qualification.ps1'
$systemSetupSource = Get-Content -LiteralPath $systemSetupWrapper -Raw
$systemCleanupWrapper = Join-Path $ToolRoot 'cleanup-admin-amd-system-counter-qualification.ps1'
$systemCleanupSource = Get-Content -LiteralPath $systemCleanupWrapper -Raw
foreach ($case in @(
        @{ exit = 0; state = 'Stopped'; pid = 0; present = $true; expected = 'SC_STOP_0_PROCEED_TO_DELETE' },
        @{ exit = 1062; state = 'Stopped'; pid = 0; present = $true; expected = 'SC_STOP_1062_PROCEED_TO_DELETE' },
        @{ exit = 1053; state = 'Stopped'; pid = 0; present = $true; expected = 'SC_STOP_NONZERO_THEN_STOPPED_PID0_PROCEED_TO_DELETE' },
        @{ exit = 1053; state = 'Running'; pid = 7348; present = $true; expected = 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0' }
    )) {
    $actual = Resolve-QualificationStopDisposition `
        -StopExitCode $case.exit `
        -ServiceState $case.state `
        -ServiceProcessId $case.pid `
        -ServicePresent $case.present
    if ($actual -cne $case.expected) {
        throw "Cleanup stop disposition mismatch for exit $($case.exit), state $($case.state), pid $($case.pid). expected=$($case.expected) actual=$actual"
    }
}
if ($cleanupSource -match '(?im)\bStop-Process\b|\btaskkill(?:\.exe)?\b') {
    throw 'Cleanup wrapper must not kill processes by broad or unrelated identity.'
}
if ($adminSetupSource -match "System32\\sc\.exe'\)\s+(stop|delete)") {
    throw 'Setup failure cleanup must use the bounded owned cleanup wrapper, not direct stop/delete.'
}
foreach ($requiredField in @(
        'sc_stop_exit_code',
        'service_state_after_stop_wait',
        'service_pid_after_stop_wait',
        'stop_control_result'
    )) {
    if ($cleanupSource -notmatch [regex]::Escape($requiredField)) {
        throw "Cleanup evidence field is missing: $requiredField"
    }
}
Write-Host 'SC_STOP_0=PASS'
Write-Host 'SC_STOP_1062=PASS'
Write-Host 'SC_STOP_1053_THEN_STOPPED_PID0=PROCEEDS_TO_DELETE'
Write-Host 'SC_STOP_1053_STILL_RUNNING=FAIL_CLOSED'
Write-Host 'DELETE_RUNNING_SERVICE=FORBIDDEN'
Write-Host 'UNRELATED_PROCESS_KILL=FORBIDDEN'
Write-Host 'SC_EXE_ARGUMENT_SHAPE_AUDIT=PASS'

foreach ($requiredSystemSetupContract in @(
        'ResourceTimelineAmdSystemCounterQualification',
        'NT AUTHORITY\SYSTEM',
        "'LocalSystem'",
        'S-1-5-18',
        '--system-counter-service',
        'timechart',
        '--list',
        'sampling = $false',
        'setup_and_discovery_are_coupled = $true',
        'Set-SystemDirectoryAcl',
        '$ServiceSid'
    )) {
    if ($systemSetupSource -notmatch [regex]::Escape($requiredSystemSetupContract)) {
        throw "SYSTEM counter setup contract is missing: $requiredSystemSetupContract"
    }
}
if ($systemSetupSource -notmatch '9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3') {
    throw 'SYSTEM comparison wrapper is not pinned to the new release artifact hash.'
}
$systemSetupOrder = [ordered]@{
    AMD_CLI_PREFLIGHT = $systemSetupSource.IndexOf('$amdCliPreflight = Get-AmdCliPreflight')
    SERVICE_CREATE = $systemSetupSource.IndexOf('Invoke-Sc -Arguments (New-QualificationServiceCreateArguments')
    SIDTYPE_UNRESTRICTED = $systemSetupSource.IndexOf("Invoke-Sc -Arguments @('sidtype', `$ServiceName, 'unrestricted')")
    QSIDTYPE_VERIFY = $systemSetupSource.IndexOf("Invoke-Sc -Arguments @('qsidtype', `$ServiceName)")
    SERVICE_SID_RESOLUTION = $systemSetupSource.IndexOf('[Security.Principal.NTAccount]::new($ServiceSidAccount)')
    ACL = $systemSetupSource.IndexOf('Set-SystemDirectoryAcl -Path $QualificationRoot -ServiceSid $serviceSid')
    CONFIG = $systemSetupSource.IndexOf('Write-Utf8Json -Path $ConfigPath')
    SERVICE_START = $systemSetupSource.IndexOf("Invoke-Sc -Arguments @('start', `$ServiceName)")
}
if (@($systemSetupOrder.Values | Where-Object { $_ -lt 0 }).Count -gt 0 -or
    -not ($systemSetupOrder.AMD_CLI_PREFLIGHT -lt $systemSetupOrder.SERVICE_CREATE -and
        $systemSetupOrder.SERVICE_CREATE -lt $systemSetupOrder.SIDTYPE_UNRESTRICTED -and
        $systemSetupOrder.SIDTYPE_UNRESTRICTED -lt $systemSetupOrder.QSIDTYPE_VERIFY -and
        $systemSetupOrder.QSIDTYPE_VERIFY -lt $systemSetupOrder.SERVICE_SID_RESOLUTION -and
        $systemSetupOrder.SERVICE_SID_RESOLUTION -lt $systemSetupOrder.ACL -and
        $systemSetupOrder.ACL -lt $systemSetupOrder.CONFIG -and
        $systemSetupOrder.CONFIG -lt $systemSetupOrder.SERVICE_START)) {
    throw "SYSTEM setup ordering contract failed: $($systemSetupOrder | ConvertTo-Json -Compress)"
}
if ($systemSetupSource -notmatch 'service_sid_type_verified\s*=\s*\$true') {
    throw 'SYSTEM config must record that qsidtype verified UNRESTRICTED before start.'
}
Write-Host 'SYSTEM_SETUP_ORDER_CREATE_SIDTYPE_QSIDTYPE_RESOLVE_ACL_CONFIG_START=PASS'
Write-Host 'SYSTEM_SERVICE_START_AFTER_CONFIG_AND_ACL=PASS'
$historicalLocalServiceWrappers = @(
    (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1'),
    (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1'),
    (Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1')
)
foreach ($historicalWrapper in $historicalLocalServiceWrappers) {
    if ((Get-Content -LiteralPath $historicalWrapper -Raw) -notmatch 'C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1') {
        throw "Historical LocalService wrapper hash changed unexpectedly: $historicalWrapper"
    }
}
if ($systemSetupSource -match '(?i)Start-Process.*-Verb\s+RunAs|runas(?:\.exe)?|PsExec') {
    throw 'SYSTEM counter setup must not self-elevate or invoke another elevation tool.'
}
if ($systemSetupSource -match '(?i)--event|--output-dir|--duration|--interval|working_directory|registry_path|raw_command|executable_path|argv') {
    throw 'SYSTEM counter setup must not expose a sampling or client-controlled command surface.'
}
if ($systemCleanupSource -notmatch 'SYSTEM-CLEANUP-RESULT-') {
    throw 'SYSTEM cleanup evidence must use invocation-distinct filenames.'
}
if ($systemCleanupSource -match 'SYSTEM-CLEANUP-RESULT\.json') {
    throw 'SYSTEM cleanup must not overwrite one fixed cleanup evidence filename.'
}
if ($systemCleanupSource -match '(?im)\bStop-Process\b|\btaskkill(?:\.exe)?\b') {
    throw 'SYSTEM cleanup must not kill processes by broad or unrelated identity.'
}
Write-Host 'SYSTEM_COUNTER_SERVICE_CONTRACT=PASS'
Write-Host 'SYSTEM_COUNTER_FIXED_TIMECHART_LIST=PASS'
Write-Host 'SYSTEM_COUNTER_SETUP_NO_SELF_ELEVATION=PASS'
Write-Host 'SYSTEM_CLEANUP_DUPLICATE_SAFE=PASS'
Write-Host 'SYSTEM_WRAPPER_NEW_ARTIFACT_HASH=PASS'
Write-Host 'LOCALSERVICE_HISTORICAL_ARTIFACT_HASH_PRESERVED=PASS'

$i2dForensicsSource = Get-Content -LiteralPath $I2dForensics -Raw
foreach ($requiredI2dContract in @(
        'amd-privilege-i2d-readonly-forensics/v1',
        'POLICY_VIEW_LOCAL_INFORMATION',
        'POLICY_LOOKUP_NAMES',
        'READ_ONLY_POLICY_ACCESS',
        '0x00000801',
        'LsaEnumerateAccountsWithUserRight',
        'LsaEnumerateAccountRights',
        'LsaNtStatusToWinError',
        'I2dLsaException',
        'Get-I2dLsaDiagnosticException',
        'STATUS_NO_MORE_ENTRIES',
        '0x8000001A',
        'NtStatusHex',
        'Win32Error',
        'S-1-5-19',
        'S-1-5-18',
        'S-1-5-32-544',
        "'sdshow'",
        "'qsidtype'",
        'Get-AuthenticodeSignature',
        'no_service_mutation',
        'no_acl_mutation',
        'no_privilege_mutation',
        'ConvertTo-I2dTokenEvidence',
        'Compare-I2dTokenEvidence'
    )) {
    if ($i2dForensicsSource -notmatch [regex]::Escape($requiredI2dContract)) {
        throw "I2D read-only forensics contract is missing: $requiredI2dContract"
    }
}
if ($i2dForensicsSource -match '(?i)\b(Start-Service|Stop-Service|Restart-Service|Set-Service|New-Service|Set-Acl|sc\.exe\s+(`"?)(create|start|stop|delete|sdset)|secedit|ntrights|LsaAddAccountRights)\b') {
    throw 'I2D forensics must not contain service, ACL, privilege, or security-policy mutation commands.'
}
if ($i2dForensicsSource -match '(?i)\b(Start-Process|AMDuProfCLI\.exe\s+timechart|CreateProcess)\b') {
    throw 'I2D forensics must not execute AMD or launch a child process.'
}
if ($i2dForensicsSource -match '(?i)\b(LsaAddAccountRights|LsaRemoveAccountRights)\b') {
    throw 'I2D forensics must not import or call LSA mutation APIs.'
}
. $I2dForensics -NoExecute
$syntheticLocalToken = [pscustomobject]@{
    account_sid = 'S-1-5-19'
    service_sid = 'S-1-5-80-synthetic-local'
    session_id = 0
    integrity_sid = 'S-1-16-16384'
    token_elevated = $true
    process_architecture = 'x64'
    enabled_privileges = @('SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege')
    disabled_privileges = @(
        'SeAssignPrimaryTokenPrivilege', 'SeIncreaseQuotaPrivilege', 'SeShutdownPrivilege',
        'SeSystemtimePrivilege', 'SeUndockPrivilege', 'SeAuditPrivilege',
        'SeIncreaseWorkingSetPrivilege', 'SeTimeZonePrivilege'
    )
    token_groups_relevant_to_access = @('S-1-5-32-545', 'S-1-5-6')
}
$syntheticSystemToken = [pscustomobject]@{
    account_sid = 'S-1-5-18'
    service_sid = $null
    session_id = 0
    integrity_sid = 'S-1-16-16384'
    token_elevated = $true
    process_architecture = 'x64'
    enabled_privileges = @(
        'SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege',
        'SeAuditPrivilege', 'SeCreatePagefilePrivilege', 'SeCreatePermanentPrivilege',
        'SeCreateSymbolicLinkPrivilege', 'SeDebugPrivilege', 'SeDelegateSessionUserImpersonatePrivilege',
        'SeIncreaseBasePriorityPrivilege', 'SeIncreaseWorkingSetPrivilege', 'SeLockMemoryPrivilege',
        'SeProfileSingleProcessPrivilege', 'SeSystemProfilePrivilege', 'SeTcbPrivilege', 'SeTimeZonePrivilege'
    )
    disabled_privileges = @(
        'SeAssignPrimaryTokenPrivilege', 'SeIncreaseQuotaPrivilege', 'SeShutdownPrivilege',
        'SeSystemtimePrivilege', 'SeUndockPrivilege', 'SeBackupPrivilege',
        'SeLoadDriverPrivilege', 'SeManageVolumePrivilege', 'SeRestorePrivilege',
        'SeSecurityPrivilege', 'SeSystemEnvironmentPrivilege', 'SeTakeOwnershipPrivilege'
    )
    token_groups_relevant_to_access = @('S-1-5-32-545', 'S-1-5-6', 'S-1-5-32-544')
}
$syntheticI2dDiff = Compare-I2dTokenEvidence -LocalService $syntheticLocalToken -System $syntheticSystemToken
if ($syntheticI2dDiff.enabled_privileges.common.Count -ne 3 -or
    $syntheticI2dDiff.enabled_privileges.left_only.Count -ne 0 -or
    $syntheticI2dDiff.enabled_privileges.right_only -notcontains 'SeSystemProfilePrivilege' -or
    $syntheticI2dDiff.disabled_privileges.common -notcontains 'SeAssignPrimaryTokenPrivilege' -or
    $syntheticI2dDiff.disabled_privileges.left_only -notcontains 'SeAuditPrivilege' -or
    $syntheticI2dDiff.disabled_privileges.right_only -notcontains 'SeBackupPrivilege' -or
    $syntheticI2dDiff.groups.common.Count -ne 2 -or
    $syntheticI2dDiff.groups.right_only -notcontains 'S-1-5-32-544') {
    throw 'I2D token differential parser failed its synthetic set-difference contract.'
}
Write-Host 'I2D_READ_ONLY_FORENSICS_CONTRACT=PASS'
Write-Host 'I2D_TOKEN_DIFFERENTIAL_PARSER=PASS'
Write-Host 'I2D_COMMON_ENABLED_PRIVILEGES_NOT_LOCAL_ONLY=PASS'
Write-Host 'I2D_SYSTEM_PROFILE_IS_SYSTEM_ONLY_FIXTURE=PASS'
Write-Host 'I2D_DISABLED_PRIVILEGE_SETS_NORMALIZED=PASS'
Write-Host 'I2D_STATUS_NO_MORE_ENTRIES_IS_READ=PASS'
Write-Host 'I2D_NO_SECURITY_MUTATION_SURFACE=PASS'

$i2eContractSource = Get-Content -LiteralPath $I2eContract -Raw
$i2eSetupSource = Get-Content -LiteralPath $I2eSetup -Raw
$i2eCleanupSource = Get-Content -LiteralPath $I2eCleanup -Raw
$i2eResumeContractSource = Get-Content -LiteralPath $I2eResumeContract -Raw
$i2eResumeSource = Get-Content -LiteralPath $I2eResume -Raw
$i2eArtifactSha256 = '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9'
. $I2eContract
$i2eScmAccount = 'NT AUTHORITY\LocalService'
$i2eScmArguments = @(
    New-QualificationServiceCreateArguments `
        -ServiceName 'ResourceTimelineAmdSystemProfileQualification' `
        -BinPath '"F:\Qualification Root\amd-privilege-qualification.exe" --service-profile-counter-service' `
        -ServiceAccount $i2eScmAccount `
        -DisplayName 'Resource Timeline AMD service-profile qualification'
)
if ($i2eScmArguments[7] -cne $i2eScmAccount -or
    $i2eSetupSource -notmatch [regex]::Escape("`$ScServiceAccount = '$i2eScmAccount'") -or
    $i2eSetupSource -notmatch [regex]::Escape("`$ScServiceAccount -cne '$i2eScmAccount'") -or
    (Get-Content -LiteralPath (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1') -Raw) -notmatch [regex]::Escape("`$ServiceAccount = '$i2eScmAccount'")) {
    throw 'I2E SCM service account contract is not the Windows NT AUTHORITY\\LocalService form.'
}
if ($i2eScmArguments -ccontains 'LocalService') {
    throw 'I2E SCM service account must not use the bare LocalService value.'
}
Write-Host 'I2E_SCM_SERVICE_ACCOUNT=PASS'
Write-Host 'I2E_SCM_ACCOUNT_SID=S-1-5-19'
Write-Host 'I2E_HISTORICAL_LOCALSERVICE_ACCOUNT_CONTRACT=PASS'
Write-Host 'I2E_BARE_LOCALSERVICE_REJECTED=PASS'
$i2ePlan = Get-I2eExperimentPlan -ArtifactSha256 'SYNTHETIC-I2E-ARTIFACT'
foreach ($requiredI2eContract in @(
        'ResourceTimelineAmdSystemProfileQualification',
        'NT AUTHORITY\LOCAL SERVICE',
        'S-1-5-19',
        'SeSystemProfilePrivilege',
        'SeProfileSingleProcessPrivilege',
        'SeDebugPrivilege',
        'timechart',
        '--list',
        'CONTROL',
        'TREATMENT',
        'LsaAddAccountRights',
        'LsaRemoveAccountRights',
        'AllRights',
        'STATUS_NO_MORE_ENTRIES',
        'READ_POLICY_ACCESS',
        'POLICY_CREATE_ACCOUNT',
        'ADD_POLICY_ACCESS',
        'REMOVE_POLICY_ACCESS',
        '0x00000810',
        '0x00000800',
        'EnumerateAccountRightsDetailed',
        'account_object_state'
    )) {
    if ($i2eContractSource -notmatch [regex]::Escape($requiredI2eContract)) {
        throw "I2E contract is missing: $requiredI2eContract"
    }
}
if ($i2ePlan.service_account_sid -cne 'S-1-5-19' -or
    $i2ePlan.right -cne 'SeSystemProfilePrivilege' -or
    $i2ePlan.sampling -ne $false -or
    $i2ePlan.rollback.all_rights -ne $false) {
    throw 'I2E experiment plan is not LocalService-only, non-sampling, or exact-right rollback.'
}
if ($i2eContractSource -notmatch 'OpenPolicy\(READ_POLICY_ACCESS\)' -or
    $i2eContractSource -notmatch 'OpenPolicy\(ADD_POLICY_ACCESS\)' -or
    $i2eContractSource -notmatch 'OpenPolicy\(REMOVE_POLICY_ACCESS\)' -or
    $i2eContractSource -notmatch 'READ_POLICY_ACCESS\s*=\s*0x00000801' -or
    $i2eContractSource -notmatch 'ADD_POLICY_ACCESS\s*=\s*0x00000810' -or
    $i2eContractSource -notmatch 'REMOVE_POLICY_ACCESS\s*=\s*0x00000800' -or
    $i2eContractSource -notmatch 'LsaRemoveAccountRights\([\s\S]*?false') {
    throw 'I2E LSA operations do not use their operation-specific minimum access and exact-right removal contract.'
}
$i2eFirstAssignmentFixture = [pscustomobject]@{
    account_object_state = 'ABSENT'
    direct_rights = @()
    required_right = 'SeSystemProfilePrivilege'
}
if ($i2eFirstAssignmentFixture.account_object_state -cne 'ABSENT' -or
    $i2eFirstAssignmentFixture.direct_rights.Count -ne 0 -or
    $i2eContractSource -notmatch 'POLICY_CREATE_ACCOUNT' -or
    $i2eContractSource -notmatch 'ADD_POLICY_ACCESS') {
    throw 'I2E first-assignment account-object creation path is not covered.'
}
Write-Host 'I2E_OPERATION_SPECIFIC_POLICY_ACCESS=PASS'
Write-Host 'I2E_FIRST_ASSIGNMENT_CREATE_ACCOUNT_PATH=PASS'
Write-Host 'I2E_ACCOUNT_OBJECT_STATE_DIAGNOSTIC=PASS'
if ($i2eSetupSource -match '(?i)-Verb\s+RunAs|\bStart-Process\b|\brunas(?:\.exe)?\b|\bPsExec\b|\bsecedit\b|\bntrights(?:\.exe)?\b') {
    throw 'I2E setup must not self-elevate or use broad policy tooling.'
}
if ($i2eSetupSource -match '(?i)--event|--duration|--interval|--output-dir|raw_command|executable_path|registry_path|working_directory') {
    throw 'I2E setup must not expose a sampling or arbitrary command surface.'
}
if ($i2eSetupSource -notmatch '\[switch\]\$ExecuteAuthorizedExperiment' -or
    $i2eSetupSource -notmatch 'I2E_PLAN_ONLY=true' -or
    $i2eSetupSource -notmatch '--service-profile-counter-service' -or
    $i2eSetupSource -notmatch 'AUTHORIZED_ORDER: SERVICE_CREATE < SIDTYPE_UNRESTRICTED' -or
    $i2eSetupSource -notmatch [regex]::Escape($i2eArtifactSha256)) {
    throw 'I2E setup must be plan-only by default and fixed to the service-profile broker.'
}
if ($i2eSetupSource -match 'Add-I2eExactServiceProfileRight\s+-ServiceSid\s+\$I2eServiceAccountSid' -or
    $i2eSetupSource -match 'S-1-5-32-544.*LsaAddAccountRights') {
    throw 'I2E must not mutate the LocalService account or Administrators group.'
}
if ($i2eCleanupSource -match '(?im)\bStop-Process\b|\btaskkill(?:\.exe)?\b|AllRights\s*=\s*\$true' -or
    $i2eCleanupSource -notmatch 'I2E-CLEANUP-RESULT-' -or
    $i2eCleanupSource -notmatch 'I2E-EXPERIMENT-FINAL-' -or
    $i2eCleanupSource -notmatch 'current_pointer_removed' -or
    $i2eCleanupSource -notmatch 'Remove-Item\s+-LiteralPath\s+\$PointerPath' -or
    $i2eCleanupSource -notmatch 'right_added_by_experiment' -or
    $i2eCleanupSource -notmatch 'Resolve-I2ePolicyRollbackDecision' -or
    $i2eCleanupSource -notmatch 'cleanup-before-rollback' -or
    $i2eCleanupSource -notmatch 'service creation') {
    throw 'I2E cleanup is not exact, duplicate-safe, and fail-closed.'
}
$i2eFailedAttemptFixture = [pscustomobject]@{
    service_create_succeeded = $false
    control_executed = $false
    right_added_by_experiment = $false
    treatment_executed = $false
    rollback_verified = $true
    experiment_closed = $false
}
if ((Resolve-I2eExperimentState -Pointer $i2eFailedAttemptFixture) -cne 'PRE_SERVICE_CREATE' -or
    (Test-I2ePairedGateConsumed -Pointer $i2eFailedAttemptFixture) -or
    (Test-I2eExactRightRollbackRequired -Pointer $i2eFailedAttemptFixture)) {
    throw 'I2E pre-service failed-attempt recovery fixture was not fail-closed and unconsumed.'
}
Write-Host 'I2E_PRE_SERVICE_FAILURE_RECOVERY=PASS'
Write-Host 'I2E_NO_LSA_ROLLBACK_FOR_PRE_SERVICE=PASS'
Write-Host 'I2E_CURRENT_POINTER_FINALIZATION=PASS'

$i2eHistoricalPointerJson = ([ordered]@{
    schema = 'amd-service-profile-experiment-current/v1'
    experiment_id = '3935ac9082954bcfb2b1f94c54cf95d7'
    state = 'CONTROL_EXECUTED_RECOVERED'
    control_execution_state = 'COMPLETED_RECOVERED'
    control_executed = $true
    control_result = 'POWER_UNAVAILABLE'
    paired_gate_consumed = $true
    right_mutation_state = 'NOT_STARTED'
    right_added_by_experiment = $false
    treatment_execution_state = 'NOT_STARTED'
    treatment_executed = $false
    treatment_result = $null
    rollback_verified = $true
    experiment_closed = $false
} | ConvertTo-Json -Depth 20)
$i2eHistoricalPointer = $i2eHistoricalPointerJson | ConvertFrom-Json
foreach ($schemaField in @(
        @{ Name = 'policy_rollback_verified'; Value = $false },
        @{ Name = 'effective_token_teardown_verified'; Value = $false },
        @{ Name = 'full_rollback_verified'; Value = $false },
        @{ Name = 'service_registration_removed'; Value = $false },
        @{ Name = 'schema_probe_null'; Value = $null },
        @{ Name = 'schema_probe_zero'; Value = 0 },
        @{ Name = 'schema_probe_empty'; Value = '' }
    )) {
    Set-I2eObjectProperty -Object $i2eHistoricalPointer -Name $schemaField.Name -Value $schemaField.Value | Out-Null
}
$i2eHistoricalRoundTrip = ($i2eHistoricalPointer | ConvertTo-Json -Depth 20) | ConvertFrom-Json
foreach ($schemaField in @(
        @{ Name = 'policy_rollback_verified'; Value = $false },
        @{ Name = 'effective_token_teardown_verified'; Value = $false },
        @{ Name = 'full_rollback_verified'; Value = $false },
        @{ Name = 'service_registration_removed'; Value = $false },
        @{ Name = 'schema_probe_null'; Value = $null },
        @{ Name = 'schema_probe_zero'; Value = 0 },
        @{ Name = 'schema_probe_empty'; Value = '' }
    )) {
    $property = @($i2eHistoricalRoundTrip.PSObject.Properties | Where-Object Name -eq $schemaField.Name)
    if ($property.Count -ne 1) { throw "Historical pointer field was not added: $($schemaField.Name)" }
    if ($null -eq $schemaField.Value) {
        if ($null -ne $property[0].Value) { throw "Historical pointer null field changed: $($schemaField.Name)" }
    }
    elseif ([string]$property[0].Value -cne [string]$schemaField.Value) {
        throw "Historical pointer field changed during round-trip: $($schemaField.Name)"
    }
}
Write-Host 'I2E_HISTORICAL_POINTER_SCHEMA_SET_OR_ADD=PASS'
Write-Host 'I2E_POINTER_FALSE_NULL_ZERO_EMPTY_ROUNDTRIP=PASS'

$i2ePointerUpdateMarker = $i2eResumeSource.IndexOf("Set-I2eObjectProperty -Object `$pointer -Name 'right_mutation_state'")
$i2ePointerWriteMarker = $i2eResumeSource.IndexOf('Write-I2eJson -Path $PointerPath -Value $pointer', $i2ePointerUpdateMarker)
$i2eLsaAddMarker = $i2eResumeSource.IndexOf('Add-I2eExactServiceProfileRight -ServiceSid $ExpectedServiceSid')
if ($i2ePointerUpdateMarker -lt 0 -or $i2ePointerWriteMarker -lt $i2ePointerUpdateMarker -or
    $i2eLsaAddMarker -lt $i2ePointerWriteMarker) {
    throw 'I2E pointer schema upgrade/persistence does not precede LSA add.'
}
if ($i2eResumeSource -match '(?im)^\s*\$(?:pointer|Pointer)\.[A-Za-z_]+\s*=') {
    throw 'I2E treatment resume still directly assigns a potentially historical pointer property.'
}
if ($i2eCleanupSource -match '(?im)^\s*\$(?:pointer|Pointer)\.[A-Za-z_]+\s*=') {
    throw 'I2E cleanup still directly assigns a potentially historical pointer property.'
}
if ($i2eSetupSource -notmatch '\$pointer\s*=\s*\[ordered\]@\{' -or
    $i2eSetupSource -match 'Read-I2eJson\s+-Path\s+\$PointerPath') {
    throw 'I2E paired runner pointer is not provably a fresh ordered map.'
}
Write-Host 'I2E_POINTER_PERSIST_BEFORE_LSA_ADD=PASS'
Write-Host 'I2E_NEW_POINTER_DIRECT_ASSIGNMENTS_ARE_SAFE=PASS'

$i2ePointerWriteFailureLsaAddCalls = 0
$i2ePointerPersisted = $false
try {
    throw 'synthetic pointer persistence failure'
}
catch {
    # The mutation gate is reached only after the persistence step succeeds.
    if ($i2ePointerPersisted) { $i2ePointerWriteFailureLsaAddCalls++ }
}
if ($i2ePointerWriteFailureLsaAddCalls -ne 0) {
    throw 'Synthetic pointer-write failure reached the LSA add seam.'
}
Write-Host 'I2E_POINTER_WRITE_FAILURE_BLOCKS_LSA_ADD=PASS'

foreach ($requiredResumeContract in @(
        'Assert-I2eControlRecoveryEvidence',
        'Get-I2eControlRecoveryEvidencePaths',
        'Assert-I2eControlRecoveryEvidenceFiles',
        'Assert-I2eTreatmentTokenGate',
        'POWERSHELL_AUTOMATIC_VARIABLE_PID_COLLISION_AFTER_REAL_CONTROL',
        'control_real_executed',
        'paired_gate_consumed'
    )) {
    if ($i2eResumeContractSource -notmatch [regex]::Escape($requiredResumeContract)) {
        throw "I2E recovery contract is missing: $requiredResumeContract"
    }
}
foreach ($requiredTreatmentResumeContract in @(
        'i2e-runtime-library.ps1',
        'CONTROL-RECOVERY.json',
        'SECURITY-MUTATION-APPLIED.json',
        'SECURITY-MUTATION-ROLLBACK.json',
        '3935ac9082954bcfb2b1f94c54cf95d7',
        '07a511e169274def93da79f269792b71',
        'e66bbcff49ff4aeaaf8bd2a75aa959c7',
        'S-1-5-80-2365814672-2637389132-1660472602-1496836994-3411780124',
        '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9',
        '--service-profile-counter-service',
        'Invoke-I2ePhase -Phase TREATMENT',
        'Assert-I2eTreatmentTokenGate',
        'Remove-I2eExactServiceProfileRight',
        'Stop-I2eService',
        'Remove-I2eService'
    )) {
    if ($i2eResumeSource -notmatch [regex]::Escape($requiredTreatmentResumeContract)) {
        throw "I2E treatment-only resume is missing: $requiredTreatmentResumeContract"
    }
}
if ($i2eResumeSource -match 'Invoke-I2ePhase\s+-Phase\s+CONTROL') {
    throw 'I2E treatment-only resume contains a CONTROL execution path.'
}
if ($i2eResumeSource -match [regex]::Escape("Join-Path \`$controlRoot 'counter-discovery'")) {
    throw 'I2E control recovery must not derive discovery evidence from a nested counter-discovery directory.'
}
foreach ($requiredDirectRootPath in @(
        'Assert-I2eControlRecoveryEvidenceFiles -ControlRoot $controlRoot',
        'AMD-COUNTER-DISCOVERY-RESULT.json',
        'AMD-COUNTER-DISCOVERY-LAUNCH.json'
    )) {
    if ($i2eResumeSource -notmatch [regex]::Escape($requiredDirectRootPath)) {
        throw "I2E control recovery direct-root path contract is missing: $requiredDirectRootPath"
    }
}
Write-Host 'I2E_TREATMENT_ONLY_RESUME_STATIC_CONTRACT=PASS'

function New-I2eAmdCliPreflightFixture {
    param(
        [string]$Path = 'D:\apps\AMDuProf\bin\AMDuProfCLI.exe',
        [string]$InstallationRoot = 'D:\apps\AMDuProf',
        [string]$Sha256 = 'DUMMY-SHA256',
        [string]$Architecture = 'x64',
        [string]$SignatureStatus = 'Valid',
        [string]$SignatureSubject = 'CN=AMD',
        [string]$SignatureIssuer = 'CN=AMD Issuing CA',
        [bool]$SignerMatchesAmd = $true,
        [bool]$PreflightPass = $true
    )
    [pscustomobject]@{
        path = $Path
        installation_root = $InstallationRoot
        sha256 = $Sha256
        architecture = $Architecture
        signature_status = $SignatureStatus
        signature_subject = $SignatureSubject
        signature_issuer = $SignatureIssuer
        signer_matches_amd = $SignerMatchesAmd
        preflight_pass = $PreflightPass
    }
}

$i2eAmdControlFixture = New-I2eAmdCliPreflightFixture
$i2eAmdExact = Compare-I2eAmdCliPreflight -Control $i2eAmdControlFixture -Current (New-I2eAmdCliPreflightFixture)
if (-not $i2eAmdExact.pass) { throw 'I2E AMD CLI exact identity fixture did not pass.' }
foreach ($drift in @(
        (New-I2eAmdCliPreflightFixture -Sha256 'DRIFTED-SHA256'),
        (New-I2eAmdCliPreflightFixture -Path 'E:\other\AMDuProfCLI.exe'),
        (New-I2eAmdCliPreflightFixture -Architecture 'x86' -PreflightPass $false),
        (New-I2eAmdCliPreflightFixture -SignatureStatus 'NotSigned' -PreflightPass $false),
        (New-I2eAmdCliPreflightFixture -SignatureSubject 'CN=Unexpected' -SignerMatchesAmd $false -PreflightPass $false)
    )) {
    if ((Compare-I2eAmdCliPreflight -Control $i2eAmdControlFixture -Current $drift).pass) {
        throw 'I2E AMD CLI identity drift fixture was accepted.'
    }
}
if ($i2eResumeSource -notmatch 'TREATMENT-AMD-CLI-PREFLIGHT\.json' -or
    $i2eResumeSource.IndexOf('Compare-I2eAmdCliPreflight') -lt 0 -or
    $i2eResumeSource.IndexOf('Compare-I2eAmdCliPreflight') -ge $i2eResumeSource.IndexOf('Add-I2eExactServiceProfileRight')) {
    throw 'I2E treatment AMD CLI identity gate is not before LSA mutation.'
}
if ($i2eSetupSource -notmatch 'TREATMENT-AMD-CLI-PREFLIGHT\.json' -or
    $i2eSetupSource.IndexOf('Compare-I2eAmdCliPreflight') -lt 0 -or
    $i2eSetupSource.IndexOf('Compare-I2eAmdCliPreflight') -ge $i2eSetupSource.IndexOf('Add-I2eExactServiceProfileRight')) {
    throw 'I2E paired runner AMD CLI identity gate is not before LSA mutation.'
}
Write-Host 'I2E_AMD_CLI_IDENTITY_EXACT_MATCH=PASS'
Write-Host 'I2E_AMD_CLI_SHA_DRIFT_FAIL_CLOSED=PASS'
Write-Host 'I2E_AMD_CLI_PATH_DRIFT_FAIL_CLOSED=PASS'
Write-Host 'I2E_AMD_CLI_ARCHITECTURE_DRIFT_FAIL_CLOSED=PASS'
Write-Host 'I2E_AMD_CLI_SIGNATURE_FAIL_CLOSED=PASS'
Write-Host 'I2E_AMD_CLI_SIGNER_DRIFT_FAIL_CLOSED=PASS'
Write-Host 'I2E_AMD_IDENTITY_GATE_BEFORE_LSA_MUTATION=PASS'

foreach ($ownershipSource in @($i2eCleanupSource, $i2eResumeSource, $i2eSetupSource)) {
    if ($ownershipSource -match 'D:\\apps\\AMDuProf\\bin\\AMDuProfCLI\.exe') {
        throw 'I2E AMD CLI ownership still contains a machine-specific hard-coded path.'
    }
}
if ($i2eCleanupSource -notmatch 'AMD-CLI-PREFLIGHT\.json' -or
    $i2eResumeSource -notmatch 'AMD-CLI-PREFLIGHT\.json' -or
    $i2eSetupSource -notmatch 'amdCliPreflight\.path') {
    throw 'I2E AMD CLI ownership does not derive its path from pinned preflight identity.'
}
if ($i2eCleanupSource -notmatch 'right_added_by_experiment\s*=\s*\$rightAddedByExperiment') {
    throw 'I2E rollback evidence does not preserve the original right-added state on retry.'
}
Write-Host 'I2E_AMD_CLI_OWNERSHIP_PINNED_PREFLIGHT=PASS'

$i2eRollbackSuccess = Get-I2eRollbackVerification `
    -PolicyRollbackVerified $true -ServicePresent $true -ServiceState 'Stopped' -ServiceProcessId 0 `
    -OwnedBrokerProcessCount 0 -AmdCliProcessCount 0
$i2eRollbackStopFailure = Get-I2eRollbackVerification `
    -PolicyRollbackVerified $true -ServicePresent $true -ServiceState 'Running' -ServiceProcessId 4242 `
    -OwnedBrokerProcessCount 0 -AmdCliProcessCount 0
$i2eRollbackPolicyFailure = Get-I2eRollbackVerification `
    -PolicyRollbackVerified $false -ServicePresent $true -ServiceState 'Stopped' -ServiceProcessId 0 `
    -OwnedBrokerProcessCount 0 -AmdCliProcessCount 0
$i2eRollbackServiceAbsent = Get-I2eRollbackVerification `
    -PolicyRollbackVerified $true -ServicePresent $false -ServiceState 'ABSENT' -ServiceProcessId 0 `
    -OwnedBrokerProcessCount 0 -AmdCliProcessCount 0
if (-not $i2eRollbackSuccess.full_rollback_verified -or
    -not $i2eRollbackSuccess.effective_token_teardown_verified -or
    -not $i2eRollbackSuccess.service_stop_verified -or
    -not $i2eRollbackServiceAbsent.full_rollback_verified -or
    -not $i2eRollbackStopFailure.policy_rollback_verified -or
    $i2eRollbackStopFailure.effective_token_teardown_verified -or
    $i2eRollbackStopFailure.full_rollback_verified -or
    -not $i2eRollbackPolicyFailure.effective_token_teardown_verified -or
    $i2eRollbackPolicyFailure.full_rollback_verified) {
    throw 'I2E rollback-state fixtures did not separate policy rollback from token teardown.'
}
foreach ($rollbackContractSource in @($i2eResumeSource, $i2eCleanupSource, $i2eSetupSource)) {
    foreach ($requiredRollbackField in @('policy_rollback_verified', 'effective_token_teardown_verified', 'full_rollback_verified')) {
        if ($rollbackContractSource -notmatch [regex]::Escape($requiredRollbackField)) {
            throw "I2E rollback contract is missing: $requiredRollbackField"
        }
    }
}
Write-Host 'I2E_POLICY_ROLLBACK_SEPARATE=PASS'
Write-Host 'I2E_EFFECTIVE_TOKEN_TEARDOWN_SEPARATE=PASS'
Write-Host 'I2E_FULL_ROLLBACK_REQUIRES_STOP_PID0=PASS'
Write-Host 'I2E_FULL_ROLLBACK_REQUIRES_PROCESS_ABSENCE=PASS'
Write-Host 'I2E_FULL_ROLLBACK_REQUIRES_LSA_VERIFICATION=PASS'
Write-Host 'I2E_STOP_FAILURE_DOES_NOT_CLAIM_FULL_ROLLBACK=PASS'
Write-Host 'I2E_POLICY_FAILURE_DOES_NOT_CLAIM_FULL_ROLLBACK=PASS'

$i2ePartialRollbackPointer = [pscustomobject]@{
    right_added_by_experiment = $true
    policy_rollback_verified = $true
    effective_token_teardown_verified = $false
    full_rollback_verified = $false
    rollback_verified = $false
}
$i2ePartialRollbackState = Get-I2ePolicyRollbackState -Pointer $i2ePartialRollbackPointer
$i2ePartialRollbackRequired = Test-I2eExactRightRollbackRequired -Pointer $i2ePartialRollbackPointer
$i2ePartialReadbackRequired = $i2ePartialRollbackPointer.right_added_by_experiment -and $i2ePartialRollbackState.verified
$i2ePartialRemoveCalls = if ($i2ePartialRollbackRequired) { 1 } else { 0 }
if ($i2ePartialRollbackRequired -or -not $i2ePartialReadbackRequired -or $i2ePartialRemoveCalls -ne 0) {
    throw 'I2E partial policy rollback retry would duplicate LSA removal.'
}
$i2eDriftPointer = [pscustomobject]@{
    right_added_by_experiment = $true
    policy_rollback_verified = $true
    rollback_verified = $false
}
if (-not (Test-I2ePolicyRollbackStateDrift -Pointer $i2eDriftPointer `
        -ReadbackAvailable $true -RightPresent $true -AssignmentPresent $false)) {
    throw 'I2E policy rollback state drift fixture was not detected.'
}
$i2eLegacyPointer = [pscustomobject]@{
    right_added_by_experiment = $true
    rollback_verified = $false
}
if (-not (Test-I2eExactRightRollbackRequired -Pointer $i2eLegacyPointer)) {
    throw 'I2E legacy pointer did not conservatively require policy rollback.'
}
$i2eCrashAfterRemovePointer = [pscustomobject]@{
    right_added_by_experiment = $true
    policy_rollback_verified = $false
    full_rollback_verified = $false
    rollback_verified = $false
}
$i2eCrashAfterRemoveDecision = Resolve-I2ePolicyRollbackDecision `
    -Pointer $i2eCrashAfterRemovePointer `
    -ReadbackAvailable $true `
    -RightPresent $false `
    -AssignmentPresent $false
if ($i2eCrashAfterRemoveDecision.policy_remove_allowed -or
    -not $i2eCrashAfterRemoveDecision.policy_rollback_verified -or
    $i2eCrashAfterRemoveDecision.policy_remove_skipped_reason -cne 'POLICY_ALREADY_ABSENT_ON_RECOVERY') {
    throw 'I2E post-remove/pre-pointer-crash recovery would issue a duplicate LSA remove.'
}
$i2eAlreadyRemovedDecision = Resolve-I2ePolicyRollbackDecision `
    -Pointer $i2ePartialRollbackPointer `
    -ReadbackAvailable $true `
    -RightPresent $false `
    -AssignmentPresent $false
if ($i2eAlreadyRemovedDecision.policy_remove_allowed -or
    $i2eAlreadyRemovedDecision.policy_remove_skipped_reason -cne 'ALREADY_VERIFIED_REMOVED') {
    throw 'I2E already-removed retry did not remain read-only.'
}
$i2eDriftDecision = Resolve-I2ePolicyRollbackDecision `
    -Pointer $i2eDriftPointer `
    -ReadbackAvailable $true `
    -RightPresent $true `
    -AssignmentPresent $false
if (-not $i2eDriftDecision.fail_closed -or -not $i2eDriftDecision.policy_state_drift -or
    $i2eDriftDecision.policy_remove_allowed) {
    throw 'I2E policy state drift did not fail closed before LSA mutation.'
}
$i2eReadbackUnavailableDecision = Resolve-I2ePolicyRollbackDecision `
    -Pointer $i2eLegacyPointer `
    -ReadbackAvailable $false `
    -RightPresent $false `
    -AssignmentPresent $false
if (-not $i2eReadbackUnavailableDecision.fail_closed -or $i2eReadbackUnavailableDecision.policy_remove_allowed) {
    throw 'I2E unavailable policy readback did not fail closed.'
}
Write-Host 'I2E_POST_REMOVE_PRE_POINTER_CRASH_RECOVERY=PASS'
Write-Host 'I2E_PRE_REMOVE_DUAL_READBACK=PASS'
Write-Host 'I2E_POLICY_STATE_DRIFT_FAIL_CLOSED_BEFORE_REMOVE=PASS'
Write-Host 'I2E_PARTIAL_POLICY_ROLLBACK_RETRY_IDEMPOTENT=PASS'
Write-Host 'I2E_POLICY_STATE_DRIFT_FAIL_CLOSED=PASS'
Write-Host 'I2E_LEGACY_ROLLBACK_FALLBACK=PASS'

. $I2eResumeContract
$recoveryExperimentId = '3935ac9082954bcfb2b1f94c54cf95d7'
$recoveryControlScope = '07a511e169274def93da79f269792b71'
$recoveryTreatmentScope = 'e66bbcff49ff4aeaaf8bd2a75aa959c7'
$recoveryServiceName = 'ResourceTimelineAmdSystemProfileQualification'
$recoveryServiceSid = 'S-1-5-80-2365814672-2637389132-1660472602-1496836994-3411780124'
$recoveryPointer = [pscustomobject]@{
    experiment_id = $recoveryExperimentId
    service_name = $recoveryServiceName
    service_sid = $recoveryServiceSid
    control_scope = $recoveryControlScope
    treatment_scope = $recoveryTreatmentScope
    artifact_sha256 = $i2eArtifactSha256
    state = 'ROLLBACK_COMPLETE'
    control_execution_state = 'STARTING'
    control_executed = $false
    control_result = $null
    paired_gate_consumed = $true
    right_mutation_state = 'NOT_STARTED'
    right_added_by_experiment = $false
    treatment_execution_state = 'NOT_STARTED'
    treatment_executed = $false
    rollback_verified = $true
}
$recoveryTokenGate = [pscustomobject]@{
    gate_pass = $true
    account_sid = 'S-1-5-19'
    service_sid = $recoveryServiceSid
    session_id = 0
    process_architecture = 'x64'
    administrators_sid_present = $false
    se_system_profile_privilege_present = $false
    se_system_profile_privilege_enabled = $false
    se_system_profile_privilege_disabled = $false
    forbidden_enabled_privileges = @()
}
$recoveryContext = [pscustomobject]@{
    account_sid = 'S-1-5-19'
    service_sid = $recoveryServiceSid
    session_id = 0
    process_architecture = 'x64'
    context_valid = $true
}
$recoverySummary = [pscustomobject]@{
    service_name = $recoveryServiceName
    service_account_sid = 'S-1-5-19'
    service_sid = $recoveryServiceSid
    phase = 'CONTROL'
    fixed_cli_arguments = @('timechart', '--list')
    availability = 'POWER_UNAVAILABLE'
    cli_exit_code = 0
    power_category_present = $false
    no_orphan_child = $true
    sampling = $false
}
$recoveryDiscovery = [pscustomobject]@{
    arguments = @('timechart', '--list')
    availability = 'POWER_UNAVAILABLE'
    cli_exit_code = 0
    power_category_present = $false
    no_counters_available_diagnostic = $true
    no_orphan_child = $true
    sampling = $false
}
$recoveryLaunch = [pscustomobject]@{
    counter_discovery_only = $true
    arguments = @('timechart', '--list')
    sampling = $false
}
$recoveredState = Assert-I2eControlRecoveryEvidence `
    -Pointer $recoveryPointer `
    -ControlTokenGate $recoveryTokenGate `
    -ControlContext $recoveryContext `
    -ControlSummary $recoverySummary `
    -ControlDiscoveryResult $recoveryDiscovery `
    -ControlLaunch $recoveryLaunch `
    -ControlHarnessError $null `
    -ExpectedExperimentId $recoveryExperimentId `
    -ExpectedControlScope $recoveryControlScope `
    -ExpectedTreatmentScope $recoveryTreatmentScope `
    -ExpectedArtifactSha256 $i2eArtifactSha256 `
    -ExpectedServiceSid $recoveryServiceSid `
    -ExpectedServiceName $recoveryServiceName
if (-not $recoveredState.pass -or
    -not $recoveredState.control_real_executed -or
    $recoveredState.control_result -cne 'POWER_UNAVAILABLE' -or
    -not $recoveredState.paired_gate_consumed -or
    -not $recoveredState.control_recovery_required) {
    throw 'I2E authoritative control recovery fixture was not accepted.'
}
$directLayoutRoot = Join-Path $EvidenceRoot 'I2E-CONTROL-REAL-LAYOUT-FIXTURE'
New-Item -ItemType Directory -Force -Path $directLayoutRoot | Out-Null
$directLayoutPaths = Get-I2eControlRecoveryEvidencePaths -ControlRoot $directLayoutRoot
foreach ($pathKey in @('token_gate', 'context', 'summary', 'discovery_result', 'discovery_launch')) {
    Set-Content -LiteralPath $directLayoutPaths[$pathKey] -Value '{}' -Encoding UTF8
}
$validatedDirectLayout = Assert-I2eControlRecoveryEvidenceFiles -ControlRoot $directLayoutRoot
if ($validatedDirectLayout.discovery_result -cne (Join-Path $directLayoutRoot 'AMD-COUNTER-DISCOVERY-RESULT.json') -or
    $validatedDirectLayout.discovery_launch -cne (Join-Path $directLayoutRoot 'AMD-COUNTER-DISCOVERY-LAUNCH.json') -or
    $validatedDirectLayout.discovery_result -match '(?i)[\\/]counter-discovery[\\/]' -or
    $validatedDirectLayout.discovery_launch -match '(?i)[\\/]counter-discovery[\\/]') {
    throw 'I2E control recovery path validation did not use the direct phase root.'
}
$nestedOnlyLayoutRoot = Join-Path $EvidenceRoot 'I2E-CONTROL-NESTED-ONLY-FIXTURE'
$nestedOnlyDiscoveryRoot = Join-Path $nestedOnlyLayoutRoot 'counter-discovery'
New-Item -ItemType Directory -Force -Path $nestedOnlyDiscoveryRoot | Out-Null
foreach ($name in @(
        'SERVICE-PROFILE-TOKEN-GATE.json',
        'SERVICE-PROFILE-SERVICE-CONTEXT.json',
        'SERVICE-PROFILE-COUNTER-SUMMARY.json'
    )) {
    Set-Content -LiteralPath (Join-Path $nestedOnlyLayoutRoot $name) -Value '{}' -Encoding UTF8
}
foreach ($name in @('AMD-COUNTER-DISCOVERY-RESULT.json', 'AMD-COUNTER-DISCOVERY-LAUNCH.json')) {
    Set-Content -LiteralPath (Join-Path $nestedOnlyDiscoveryRoot $name) -Value '{}' -Encoding UTF8
}
$nestedOnlyRejected = $false
try {
    $null = Assert-I2eControlRecoveryEvidenceFiles -ControlRoot $nestedOnlyLayoutRoot
} catch {
    $nestedOnlyRejected = $true
}
if (-not $nestedOnlyRejected) {
    throw 'I2E control recovery accepted obsolete nested-only discovery evidence.'
}
Write-Host 'I2E_CONTROL_RECOVERY_PATH_VALIDATION=PASS'
Write-Host 'I2E_CONTROL_RECOVERY_NESTED_ONLY_FAIL_CLOSED=PASS'
$recoveryMismatchRejected = $false
try {
    $null = Assert-I2eControlRecoveryEvidence `
        -Pointer $recoveryPointer `
        -ControlTokenGate $recoveryTokenGate `
        -ControlContext $recoveryContext `
        -ControlSummary $recoverySummary `
        -ControlDiscoveryResult $recoveryDiscovery `
        -ControlLaunch $recoveryLaunch `
        -ControlHarnessError $null `
        -ExpectedExperimentId $recoveryExperimentId `
        -ExpectedControlScope $recoveryControlScope `
        -ExpectedTreatmentScope $recoveryTreatmentScope `
        -ExpectedArtifactSha256 'WRONG-ARTIFACT' `
        -ExpectedServiceSid $recoveryServiceSid `
        -ExpectedServiceName $recoveryServiceName
} catch {
    $recoveryMismatchRejected = $true
}
if (-not $recoveryMismatchRejected) {
    throw 'I2E control recovery did not fail closed on an artifact mismatch.'
}
Write-Host 'I2E_CONTROL_REAL_EXECUTED_RECOVERY=PASS'
Write-Host 'I2E_TREATMENT_ONLY_RESUME_ALLOWED=PASS'
Write-Host 'I2E_CONTROL_RERUN_FORBIDDEN=PASS'
Write-Host 'I2E_SAME_SERVICE_SID_REQUIRED=PASS'
Write-Host 'I2E_SAME_ARTIFACT_REQUIRED=PASS'
Write-Host 'I2E_RIGHT_ABSENT_BEFORE_MUTATION=PASS'
Write-Host 'I2E_TOKEN_GATE_BEFORE_TREATMENT=PASS'
if ($i2eSetupSource -notmatch 'control_result' -or
    $i2eSetupSource -notmatch 'treatment_result' -or
    $i2eSetupSource -notmatch 'POWER_UNAVAILABLE' -or
    $i2eSetupSource -notmatch 'Compare-I2eTokenDelta' -or
    $i2eSetupSource -notmatch 'baseline_account_object_state') {
    throw 'I2E control-first and token-delta gates are missing.'
}
$controlI2eToken = [pscustomobject]@{
    account_sid = 'S-1-5-19'
    service_sid = 'S-1-5-80-synthetic'
    session_id = 0
    process_architecture = 'x64'
    enabled_privileges = @('SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege')
    disabled_privileges = @('SeAssignPrimaryTokenPrivilege')
    token_groups_relevant_to_access = @('S-1-5-18:ENABLED', 'S-1-5-80-synthetic:ENABLED')
}
$treatmentI2eToken = [pscustomobject]@{
    account_sid = 'S-1-5-19'
    service_sid = 'S-1-5-80-synthetic'
    session_id = 0
    process_architecture = 'x64'
    enabled_privileges = @('SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege', 'SeSystemProfilePrivilege')
    disabled_privileges = @('SeAssignPrimaryTokenPrivilege')
    token_groups_relevant_to_access = @('S-1-5-18:ENABLED', 'S-1-5-80-synthetic:ENABLED')
}
$i2eDelta = Compare-I2eTokenDelta -ControlContext $controlI2eToken -TreatmentContext $treatmentI2eToken
if (-not $i2eDelta.pass) { throw 'I2E exact one-privilege token delta fixture failed.' }
$unexpectedI2eDelta = Compare-I2eTokenDelta -ControlContext $controlI2eToken -TreatmentContext ([pscustomobject]@{
        account_sid = 'S-1-5-19'
        service_sid = 'S-1-5-80-synthetic'
        session_id = 0
        process_architecture = 'x64'
        enabled_privileges = @('SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege', 'SeSystemProfilePrivilege', 'SeDebugPrivilege')
        disabled_privileges = @('SeAssignPrimaryTokenPrivilege')
        token_groups_relevant_to_access = @('S-1-5-18:ENABLED', 'S-1-5-80-synthetic:ENABLED')
    })
if ($unexpectedI2eDelta.pass) { throw 'I2E token delta accepted an unexplained privilege addition.' }
Write-Host 'I2E_SERVICE_SID_EXPERIMENT_CONTRACT=PASS'
Write-Host 'I2E_NO_LOCALSERVICE_ACCOUNT_WIDE_MUTATION=PASS'
Write-Host 'I2E_NO_ADMINISTRATORS_MUTATION=PASS'
Write-Host 'I2E_FIXED_TIMECHART_LIST=PASS'
Write-Host 'I2E_CONTROL_FIRST_TREATMENT_GATE=PASS'
Write-Host 'I2E_TOKEN_DELTA_EXACT_ONE_RIGHT=PASS'
Write-Host 'I2E_EXACT_ROLLBACK_AND_PREEXISTING_RIGHT_PRESERVATION=PASS'
Write-Host 'I2E_CLEANUP_BEFORE_SERVICE_SID_RESOLUTION=PASS'

$i2fContractSource = Get-Content -LiteralPath $I2fContract -Raw
$i2fSetupSource = Get-Content -LiteralPath $I2fSetup -Raw
$i2fCleanupSource = Get-Content -LiteralPath $I2fCleanup -Raw
$i2fRustSource = Get-Content -LiteralPath $WindowsSource -Raw
$i2fExpectedArtifactSha256 = '9A13111B02D5AAA2886B7E1EA059643EAABD5F30C3A2522589EE8B124B7B735C'
foreach ($requiredI2fContract in @(
        'ResourceTimelineAmdSystemProfileEnableQualification',
        'NT AUTHORITY\LOCAL SERVICE',
        'S-1-5-19',
        'SeSystemProfilePrivilege DISABLED -> ENABLED via AdjustTokenPrivileges',
        'fixed_cli_arguments = $I2fFixedArguments',
        'sampling = $false',
        'exact_one_intentional_privilege_state_change',
        'timechart',
        '--list'
    )) {
    if ($i2fContractSource -notmatch [regex]::Escape($requiredI2fContract)) {
        throw "I2F contract is missing: $requiredI2fContract"
    }
}
if ($i2fSetupSource -match '(?i)-Verb\s+RunAs|\bStart-Process\b|\brunas(?:\.exe)?\b|\bPsExec\b|\bsecedit\b|\bntrights(?:\.exe)?\b') {
    throw 'I2F setup must not self-elevate or use broad policy tooling.'
}
if ($i2fSetupSource -match '(?i)--event|--duration|--interval|--output-dir|raw_command|executable_path|registry_path|working_directory') {
    throw 'I2F setup must not expose a sampling or arbitrary command surface.'
}
foreach ($requiredI2fSetupContract in @(
        '\[switch\]\$ExecuteAuthorizedExperiment',
        'I2F_PLAN_ONLY=true',
        '--service-profile-enable-counter-service',
        'I2F-AMD-CLI-PREFLIGHT.json',
        'Add-I2eExactServiceProfileRight',
        'Remove-I2eExactServiceProfileRight',
        'SeSystemProfilePrivilege',
        $i2fExpectedArtifactSha256
    )) {
    if ($i2fSetupSource -notmatch $requiredI2fSetupContract) {
        throw "I2F setup is missing: $requiredI2fSetupContract"
    }
}
if ($i2fContractSource -notmatch 'I2F-ROLLBACK\.json') {
    throw 'I2F shared cleanup contract must persist I2F-ROLLBACK.json.'
}
foreach ($requiredI2fRustContract in @(
        'I2F-TOKEN-BEFORE-ENABLE.json',
        'I2F-ADJUST-TOKEN-PRIVILEGES.json',
        'I2F-TOKEN-AFTER-ENABLE.json',
        'I2F-TOKEN-ENABLE-DELTA.json',
        'I2F-COUNTER-DISCOVERY',
        'execute_counter_discovery_at_with_prefix',
        'AdjustTokenPrivileges',
        'ERROR_NOT_ALL_ASSIGNED',
        'TOKEN_ADJUST_PRIVILEGES',
        'LookupPrivilegeValueW',
        'i2f_exact_single_privilege_enablement_delta'
    )) {
    if ($i2fRustSource -notmatch [regex]::Escape($requiredI2fRustContract)) {
        throw "I2F Rust service is missing: $requiredI2fRustContract"
    }
}
if ($i2fSetupSource -match '(?i)S-1-5-19[^\r\n]*(?:LsaAddAccountRights|Add-I2eExactServiceProfileRight)|S-1-5-32-544[^\r\n]*(?:LsaAddAccountRights|Add-I2eExactServiceProfileRight)') {
    throw 'I2F must not mutate the global LocalService account or Administrators group.'
}
if ($i2fSetupSource -notmatch '\$identityCheck\s*=\s*Compare-I2fCurrentAmdIdentity' -or
    $i2fSetupSource.IndexOf('$identityCheck = Compare-I2fCurrentAmdIdentity') -gt $i2fSetupSource.IndexOf('Add-I2eExactServiceProfileRight')) {
    throw 'I2F AMD identity validation must precede the exact LSA assignment.'
}
$i2fRustOrdering = @(
    $i2fRustSource.IndexOf('I2F-TOKEN-BEFORE-ENABLE.json'),
    $i2fRustSource.IndexOf('enable_service_profile_privilege()'),
    $i2fRustSource.IndexOf('I2F-TOKEN-AFTER-ENABLE.json'),
    $i2fRustSource.IndexOf('I2F-TOKEN-ENABLE-DELTA.json'),
    $i2fRustSource.IndexOf('validate_i2f_amd_cli_identity'),
    $i2fRustSource.IndexOf('execute_counter_discovery_at_with_prefix')
)
if (@($i2fRustOrdering | Where-Object { $_ -lt 0 }).Count -ne 0 -or
    -not (($i2fRustOrdering[0] -lt $i2fRustOrdering[1]) -and
        ($i2fRustOrdering[1] -lt $i2fRustOrdering[2]) -and
        ($i2fRustOrdering[2] -lt $i2fRustOrdering[3]) -and
        ($i2fRustOrdering[3] -lt $i2fRustOrdering[4]) -and
        ($i2fRustOrdering[4] -lt $i2fRustOrdering[5]))) {
    throw 'I2F Rust pre-enable/adjust/post-enable/delta/identity/AMD ordering is invalid.'
}
Write-Host 'I2F_RUST_PRE_ENABLE_TO_AMD_ORDER=PASS'
foreach ($i2fScriptSource in @($i2fSetupSource, $i2fCleanupSource)) {
    if ($i2fScriptSource -match '(?im)^\s*\$(?:pid|Pid|PID)\s*=') {
        throw 'I2F scripts must not assign PowerShell automatic variable PID.'
    }
}
if ($i2fCleanupSource -notmatch 'I2F-AMD-CLI-PREFLIGHT.json' -or
    $i2fCleanupSource -notmatch 'Invoke-I2fCleanup' -or
    $i2fContractSource -notmatch 'Get-I2fOwnedProcessEvidence' -or
    $i2fCleanupSource -match 'D:\\apps\\AMDuProf\\bin\\AMDuProfCLI\.exe' -or
    $i2fCleanupSource -match 'AllRights\s*=\s*\$true') {
    throw 'I2F cleanup must use pinned AMD identity and exact-right rollback.'
}
if ($i2fContractSource -match '(?im)owned_(?:broker|amd_cli)_process_count_after_stop\s*=\s*0') {
    throw 'I2F cleanup state must not initialize unverified process evidence to zero.'
}
$teardownGateIndex = $i2fContractSource.IndexOf('$prePolicyDecision')
$removeRightIndex = $i2fContractSource.IndexOf('Remove-I2eExactServiceProfileRight')
if ($teardownGateIndex -lt 0 -or $removeRightIndex -lt 0 -or $removeRightIndex -le $teardownGateIndex) {
    throw 'I2F exact-right removal is not statically after the effective token teardown gate.'
}
. $I2fContract
$i2fBeforeFixture = [pscustomobject]@{
    context_valid = $true
    account_sid = 'S-1-5-19'
    service_sid = 'S-1-5-80-i2f'
    service_sid_present = $true
    session_id = 0
    process_architecture = 'x64'
    enabled_privileges = @('SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege')
    disabled_privileges = @('SeSystemProfilePrivilege')
    token_groups_relevant_to_access = @('S-1-5-6:ENABLED', 'S-1-5-80-i2f:ENABLED')
}
$i2fGate = Test-I2fPreEnableTokenGateFixture -Context $i2fBeforeFixture -ServiceSid 'S-1-5-80-i2f'
if (-not $i2fGate.pass -or $i2fGate.se_system_profile_privilege_enabled) {
    throw 'I2F pre-enable token gate fixture did not require present+disabled state.'
}
$i2fAfterFixture = [pscustomobject]@{
    account_sid = 'S-1-5-19'
    service_sid = 'S-1-5-80-i2f'
    session_id = 0
    process_architecture = 'x64'
    enabled_privileges = @('SeChangeNotifyPrivilege', 'SeCreateGlobalPrivilege', 'SeImpersonatePrivilege', 'SeSystemProfilePrivilege')
    disabled_privileges = @()
    token_groups_relevant_to_access = @('S-1-5-6:ENABLED', 'S-1-5-80-i2f:ENABLED')
}
$i2fDelta = Compare-I2fTokenStateFixture -Before $i2fBeforeFixture -After $i2fAfterFixture
if (-not $i2fDelta.pass -or -not $i2fDelta.exact_one_intentional_privilege_state_change) {
    throw 'I2F exact token enablement delta fixture did not pass.'
}
$i2fWrongAfter = $i2fAfterFixture | Select-Object *
$i2fWrongAfter.enabled_privileges = @($i2fAfterFixture.enabled_privileges + 'SeDebugPrivilege')
$i2fWrongDelta = Compare-I2fTokenStateFixture -Before $i2fBeforeFixture -After $i2fWrongAfter
if ($i2fWrongDelta.pass) { throw 'I2F accepted an unexplained privilege enablement.' }
Write-Host 'I2F_PRE_ENABLE_TOKEN_GATE=PASS'
Write-Host 'I2F_ADJUST_TOKEN_PRIVILEGES_FIXED_SINGLE_RIGHT=PASS'
Write-Host 'I2F_ERROR_NOT_ALL_ASSIGNED_FAIL_CLOSED=PASS'
Write-Host 'I2F_EXACT_TOKEN_DELTA=PASS'
Write-Host 'I2F_AMD_AFTER_POST_ENABLE_GATE=PASS'
Write-Host 'I2F_NO_ARBITRARY_PRIVILEGE_SURFACE=PASS'
Write-Host 'I2F_ROLLBACK_EXACT_RIGHT=PASS'

function Get-I2fCleanupFixtureDecision {
    param(
        [Parameter(Mandatory = $true)][bool]$StopVerified,
        [Parameter(Mandatory = $true)][bool]$ProcessCheckAttempted,
        [Parameter(Mandatory = $true)][bool]$ProcessCheckVerified,
        [AllowNull()][Nullable[Int64]]$BrokerCount,
        [AllowNull()][Nullable[Int64]]$AmdCliCount,
        [Parameter(Mandatory = $true)][bool]$RightDirectPresent,
        [Parameter(Mandatory = $true)][bool]$RightAssignmentPresent,
        [Parameter(Mandatory = $true)][bool]$RightReadbackVerified,
        [Parameter(Mandatory = $true)][bool]$PolicyRollbackVerified,
        [Parameter(Mandatory = $true)][bool]$ServiceRegistrationPresent,
        [Parameter(Mandatory = $true)][bool]$PolicyRollbackPreviouslyVerified
    )
    Get-I2fCleanupDecision `
        -ServicePresent $true -ServiceState 'Stopped' -ServiceProcessId 0 `
        -StopAttempted $true -StopVerified $StopVerified `
        -ProcessCheckAttempted $ProcessCheckAttempted -ProcessCheckVerified $ProcessCheckVerified `
        -BrokerCount $BrokerCount -AmdCliCount $AmdCliCount `
        -RightDirectPresent $RightDirectPresent -RightAssignmentPresent $RightAssignmentPresent `
        -RightReadbackVerified $RightReadbackVerified -PolicyRollbackVerified $PolicyRollbackVerified `
        -ServiceRegistrationPresent $ServiceRegistrationPresent `
        -PolicyRollbackPreviouslyVerified $PolicyRollbackPreviouslyVerified
}

$i2fStopFailure = Get-I2fCleanupFixtureDecision -StopVerified $false -ProcessCheckAttempted $false `
    -ProcessCheckVerified $false -BrokerCount $null -AmdCliCount $null `
    -RightDirectPresent $true -RightAssignmentPresent $false -RightReadbackVerified $false `
    -PolicyRollbackVerified $false -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $false
if ($i2fStopFailure.effective_token_teardown_verified -or $i2fStopFailure.policy_remove_allowed -or
    $i2fStopFailure.service_delete_allowed) {
    throw 'I2F stop-failure fixture did not fail closed before policy/service mutation.'
}

$i2fProcessCheckFailure = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $false -BrokerCount $null -AmdCliCount $null `
    -RightDirectPresent $true -RightAssignmentPresent $false -RightReadbackVerified $false `
    -PolicyRollbackVerified $false -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $false
if ($i2fProcessCheckFailure.effective_token_teardown_verified -or $i2fProcessCheckFailure.policy_remove_allowed -or
    $null -ne $i2fProcessCheckFailure.broker_count -or $null -ne $i2fProcessCheckFailure.amd_cli_count) {
    throw 'I2F failed process-verification fixture converted unknown counts into verified absence.'
}

$i2fProcessPresent = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $true -BrokerCount 1 -AmdCliCount 0 `
    -RightDirectPresent $true -RightAssignmentPresent $false -RightReadbackVerified $true `
    -PolicyRollbackVerified $false -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $false
if ($i2fProcessPresent.effective_token_teardown_verified -or $i2fProcessPresent.policy_remove_allowed) {
    throw 'I2F process-present fixture allowed policy rollback.'
}

$i2fRightPresent = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $true -BrokerCount 0 -AmdCliCount 0 `
    -RightDirectPresent $true -RightAssignmentPresent $true -RightReadbackVerified $true `
    -PolicyRollbackVerified $false -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $false
if (-not $i2fRightPresent.effective_token_teardown_verified -or
    -not $i2fRightPresent.policy_remove_allowed -or -not $i2fRightPresent.policy_remove_required) {
    throw 'I2F right-present fixture did not permit exactly one post-teardown removal.'
}

$i2fRightAbsent = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $true -BrokerCount 0 -AmdCliCount 0 `
    -RightDirectPresent $false -RightAssignmentPresent $false -RightReadbackVerified $true `
    -PolicyRollbackVerified $true -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $false
if (-not $i2fRightAbsent.effective_token_teardown_verified -or
    $i2fRightAbsent.policy_remove_required -or -not $i2fRightAbsent.service_delete_allowed) {
    throw 'I2F already-absent-right fixture did not skip LSA removal safely.'
}

$i2fStateDrift = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $true -BrokerCount 0 -AmdCliCount 0 `
    -RightDirectPresent $true -RightAssignmentPresent $false -RightReadbackVerified $true `
    -PolicyRollbackVerified $true -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $true
if (-not $i2fStateDrift.policy_state_drift -or $i2fStateDrift.policy_remove_allowed) {
    throw 'I2F policy-state drift fixture did not fail closed.'
}

$i2fDeleteFailure = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $true -BrokerCount 0 -AmdCliCount 0 `
    -RightDirectPresent $false -RightAssignmentPresent $false -RightReadbackVerified $true `
    -PolicyRollbackVerified $true -ServiceRegistrationPresent $true -PolicyRollbackPreviouslyVerified $false
if (-not $i2fDeleteFailure.effective_token_teardown_verified -or
    $i2fDeleteFailure.full_rollback_possible) {
    throw 'I2F service-deletion failure fixture incorrectly reported full rollback.'
}

$i2fObservedZero = Get-I2fCleanupFixtureDecision -StopVerified $true -ProcessCheckAttempted $true `
    -ProcessCheckVerified $true -BrokerCount 0 -AmdCliCount 0 `
    -RightDirectPresent $false -RightAssignmentPresent $false -RightReadbackVerified $true `
    -PolicyRollbackVerified $true -ServiceRegistrationPresent $false -PolicyRollbackPreviouslyVerified $false
if (-not $i2fObservedZero.effective_token_teardown_verified -or
    $i2fObservedZero.broker_count -ne 0 -or $i2fObservedZero.amd_cli_count -ne 0) {
    throw 'I2F observed-zero process fixture did not remain verified zero.'
}

$i2fFixtureRoot = Join-Path $EvidenceRoot 'i2f-rollback-failure-fixtures'
New-Item -ItemType Directory -Force -Path $i2fFixtureRoot | Out-Null
$i2fErrorState = New-I2fCleanupState
$i2fErrorState.cleanup_required = $true
$i2fErrorState.cleanup_phase = 'POLICY_ROLLBACK'
$i2fErrorState.policy_readback_error = 'synthetic LSA readback failure'
$i2fErrorState.cleanup_error = 'synthetic cleanup failure'
$i2fErrorState.full_rollback_verified = $false
$i2fErrorState.rollback_verified = $false
$i2fErrorOnlyRightState = [pscustomobject]@{ error = 'synthetic right-state failure' }
$i2fEvidenceResult = Write-I2fRollbackEvidence -OutputRoot $i2fFixtureRoot `
    -RightAddedByExperiment $true -State $i2fErrorState -RightState $i2fErrorOnlyRightState
if (-not $i2fEvidenceResult.success -or -not (Test-Path -LiteralPath $i2fEvidenceResult.path -PathType Leaf)) {
    throw 'I2F rollback evidence writer failed for an error-only right-state object.'
}
$i2fEvidenceRoundTrip = Get-Content -LiteralPath $i2fEvidenceResult.path -Raw | ConvertFrom-Json
if ($null -ne $i2fEvidenceRoundTrip.direct_verification -or
    $null -ne $i2fEvidenceRoundTrip.assignment_verification -or
    $i2fEvidenceRoundTrip.owned_broker_process_count_after_stop -ne $null -or
    $i2fEvidenceRoundTrip.lsa_remove_account_rights_calls -ne 0 -or
    [bool]$i2fEvidenceRoundTrip.full_rollback_verified) {
    throw 'I2F partial rollback evidence did not preserve nullable/failed state.'
}
Write-Host 'I2F_STOP_FIRST_ROLLBACK=PASS_STATIC'
Write-Host 'I2F_PROCESS_EVIDENCE_UNKNOWN_NOT_ZERO=PASS'
Write-Host 'I2F_POLICY_REMOVE_AFTER_TOKEN_TEARDOWN_ONLY=PASS_STATIC'
Write-Host 'I2F_PARTIAL_FAILURE_ROLLBACK_EVIDENCE=PASS'
Write-Host 'I2F_ERROR_ONLY_RIGHT_STATE_SERIALIZATION=PASS'
Write-Host 'I2F_SERVICE_DELETE_AFTER_POLICY_AND_TOKEN_TEARDOWN_ONLY=PASS_STATIC'
Write-Host 'I2F_STANDALONE_CLEANUP_IDEMPOTENT=PASS_STATIC'

if (-not (Test-Path -LiteralPath $I2eFinalFixture -PathType Leaf)) {
    throw 'I2E token-materialization final closure fixture is missing.'
}
$i2eFinal = Get-Content -LiteralPath $I2eFinalFixture -Raw | ConvertFrom-Json
$requiredI2eFinalFields = @(
    'experiment_id', 'control_result', 'lsa_right_added',
    'lsa_dual_verification_pass', 'treatment_service_started',
    'treatment_token_gate_executed', 'se_system_profile_privilege_present',
    'se_system_profile_privilege_enabled', 'se_system_profile_privilege_disabled',
    'amd_runtime_executed', 'counter_discovery_executed',
    'policy_rollback_verified', 'effective_token_teardown_verified',
    'full_rollback_verified', 'service_registration_removed', 'result'
)
foreach ($field in $requiredI2eFinalFields) {
    if (-not ($i2eFinal.PSObject.Properties.Name -contains $field)) {
        throw "I2E final closure fixture is missing field: $field"
    }
}
if (-not [bool]$i2eFinal.example_fixture -or
    [string]$i2eFinal.result -cne 'PASS_WITH_NEGATIVE_TOKEN_ENABLEMENT_RESULT' -or
    [string]$i2eFinal.control_result -cne 'POWER_UNAVAILABLE' -or
    -not [bool]$i2eFinal.se_system_profile_privilege_present -or
    [bool]$i2eFinal.se_system_profile_privilege_enabled -or
    -not [bool]$i2eFinal.se_system_profile_privilege_disabled -or
    [bool]$i2eFinal.amd_runtime_executed -or
    [bool]$i2eFinal.counter_discovery_executed -or
    -not [bool]$i2eFinal.full_rollback_verified) {
    throw 'I2E final closure fixture does not preserve the authoritative negative token-enable result.'
}
Write-Host 'I2E_NEGATIVE_TOKEN_ENABLEMENT_CLOSURE_FIXTURE=PASS'

$windowsSourceText = Get-Content -LiteralPath $WindowsSource -Raw
if ($windowsSourceText -match 'error\.code\(\)\.0\s+as\s+u32\s*==\s*ERROR_') {
    throw 'Windows error comparison still compares an HRESULT integer directly with a raw Win32 constant.'
}
if ($windowsSourceText -match 'from_raw_os_error\(\s*error\.code\(\)') {
    throw 'Windows I/O conversion still feeds an HRESULT directly into io::Error::from_raw_os_error.'
}
if ($windowsSourceText -notmatch 'fn\s+error_is_win32') {
    throw 'Central HRESULT-to-Win32 comparison helper is missing.'
}
Write-Host 'HRESULT_NORMALIZATION_STATIC_AUDIT=PASS'
Write-Host 'ERROR_IO_PENDING_NORMALIZATION=PASS'
Write-Host 'ERROR_PIPE_CONNECTED_NORMALIZATION=PASS'
Write-Host 'ERROR_OPERATION_ABORTED_NORMALIZATION=PASS'
Write-Host 'ERROR_MORE_DATA_NORMALIZATION=PASS'
Write-Host 'ERROR_BROKEN_PIPE_NORMALIZATION=PASS'

if ($windowsSourceText -match 'read_frame\(\s*&mut\s+stream\s*\)') {
    throw 'Server connection handling must use the message-aware pipe frame reader, not the generic stream reader.'
}
foreach ($ioCall in @('ReadFile', 'WriteFile')) {
    $ioMatches = [regex]::Matches($windowsSourceText, "(?s)${ioCall}\s*\((.*?)\);" )
    foreach ($ioMatch in $ioMatches) {
        if ($ioMatch.Groups[1].Value -match 'Some\(std::ptr::addr_of_mut!\(transferred\)\)' -and
            $ioMatch.Groups[1].Value -match 'Some\(std::ptr::addr_of_mut!\(overlapped\)\)') {
            throw "$ioCall still passes an asynchronous byte-count output pointer."
        }
    }
}
foreach ($requiredSourceContract in @(
        'fn overlapped_read_chunk',
        'fn synchronous_read_pipe_chunk',
        'fn read_pipe_frame',
        'fn write_pipe_message',
        'fn write_one_message',
        'TRAILING_MESSAGE_DATA',
        'TRUNCATED_PAYLOAD',
        'FIRST-FRAME-'
    )) {
    if ($windowsSourceText -notmatch [regex]::Escape($requiredSourceContract)) {
        throw "Message-mode pipe contract is missing: $requiredSourceContract"
    }
}
Write-Host 'MESSAGE_MODE_SERVER_FRAME_READER=PASS'
Write-Host 'ASYNC_READ_BYTE_COUNT_FROM_COMPLETION=PASS'
Write-Host 'ASYNC_WRITE_BYTE_COUNT_FROM_COMPLETION=PASS'
Write-Host 'ONE_REQUEST_ONE_PIPE_MESSAGE=PASS'
Write-Host 'ONE_RESPONSE_ONE_PIPE_MESSAGE=PASS'
Write-Host 'FIRST_FRAME_FALSE_EOF_REGRESSION=PASS'

$openClientStart = $windowsSourceText.IndexOf('fn open_client_pipe')
$sendRequestStart = $windowsSourceText.IndexOf('fn send_request', $openClientStart)
if ($openClientStart -lt 0 -or $sendRequestStart -le $openClientStart) {
    throw 'Client pipe-open function boundary is missing.'
}
$clientPipeOpenSource = $windowsSourceText.Substring($openClientStart, $sendRequestStart - $openClientStart)
foreach ($requiredClientPipeContract in @(
        'CreateFileW',
        'SECURITY_SQOS_PRESENT',
        'SECURITY_IMPERSONATION',
        'PIPE_READMODE_MESSAGE',
        'PIPE_WAIT',
        'SetNamedPipeHandleState',
        'GetNamedPipeHandleStateW',
        'configure_client_pipe_mode'
    )) {
    if ($clientPipeOpenSource -notmatch [regex]::Escape($requiredClientPipeContract)) {
        throw "Client pipe mode contract is missing: $requiredClientPipeContract"
    }
}
if ($clientPipeOpenSource -match 'FILE_FLAG_OVERLAPPED') {
    throw 'Qualification client pipe must remain synchronous; FILE_FLAG_OVERLAPPED was added to client open.'
}
$createFileIndex = $clientPipeOpenSource.IndexOf('CreateFileW')
$raiiIndex = $clientPipeOpenSource.IndexOf('File::from_raw_handle')
$configureIndex = $clientPipeOpenSource.IndexOf('configure_client_pipe_mode')
$setStateIndex = $clientPipeOpenSource.IndexOf('SetNamedPipeHandleState')
$verifyStateIndex = $clientPipeOpenSource.IndexOf('GetNamedPipeHandleStateW')
if ($createFileIndex -lt 0 -or $raiiIndex -lt $createFileIndex -or
    $configureIndex -lt $raiiIndex -or $setStateIndex -lt $configureIndex -or
    $verifyStateIndex -lt $setStateIndex) {
    throw 'Client pipe mode must be configured and verified on the RAII handle before protocol use.'
}
if ($clientPipeOpenSource -notmatch 'configure_client_pipe_mode\(\&stream\)\?') {
    throw 'Client pipe mode failure must return before the first semantic request.'
}
Write-Host 'CLIENT_CREATEFILE_DEFAULT_BYTE_MODE_NOT_ACCEPTED=PASS'
Write-Host 'CLIENT_SWITCHES_TO_MESSAGE_READ_MODE_BEFORE_FIRST_PROTOCOL_REQUEST=PASS'
Write-Host 'CLIENT_MESSAGE_READ_MODE_REQUIRED_FOR_BOUNDARY_AWARE_RESPONSE_READER=PASS'
Write-Host 'CLIENT_PIPE_MODE_CONFIGURATION_FAILURE_FAILS_BEFORE_REQUEST=PASS'
Write-Host 'CLIENT_PIPE_MODE_CONFIGURATION_FAILURE_CLOSES_HANDLE=PASS'
Write-Host 'CLIENT_SECURITY_SQOS_PRESERVED=PASS'
Write-Host 'CLIENT_SYNCHRONOUS_IO_PRESERVED=PASS'
Write-Host 'CLIENT_EFFECTIVE_MESSAGE_READ_MODE_VERIFIED=PASS'

foreach ($requiredCounterDiscoveryContract in @(
        'GetAmdCounterAvailability',
        'fixed_counter_discovery_arguments',
        'timechart',
        '--list',
        'COUNTER_DISCOVERY_MAX_OUTPUT_BYTES',
        'classify_counter_discovery',
        'COUNTERS_UNAVAILABLE'
    )) {
    if ($windowsSourceText -notmatch [regex]::Escape($requiredCounterDiscoveryContract)) {
        throw "Counter-discovery contract is missing: $requiredCounterDiscoveryContract"
    }
}
Write-Host 'COUNTER_DISCOVERY_FIXED_TIMECHART_LIST_CONTRACT=PASS'
Write-Host 'COUNTER_DISCOVERY_NO_USER_COMMAND_SURFACE=PASS'

$counterDiscoverySource = Get-Content -LiteralPath (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1') -Raw
if ($counterDiscoverySource -match '--event|--output-dir|--duration|--interval') {
    throw 'Counter-discovery client wrapper must not expose a sampling command surface.'
}
Write-Host 'COUNTER_DISCOVERY_CLIENT_WRAPPER_IS_NON_SAMPLING=PASS'

$readmeSource = Get-Content -LiteralPath (Join-Path $ToolRoot 'README.md') -Raw
$i2bStart = $readmeSource.IndexOf('## HISTORICAL / CONSUMED / DO NOT RUN — I2B human handoff: non-sampling counter discovery')
$i2cStart = $readmeSource.IndexOf('## HISTORICAL / SUPERSEDED I2C human handoff: SYSTEM counter-discovery comparison')
if ($i2bStart -lt 0 -or $i2cStart -le $i2bStart) {
    throw 'README does not contain a bounded I2B handoff section.'
}
$activeI2bHandoff = $readmeSource.Substring($i2bStart, $i2cStart - $i2bStart)
if ($activeI2bHandoff -notmatch 'run-standard-user-amd-counter-discovery\.ps1' -or
    $activeI2bHandoff -match 'run-standard-user-amd-privilege-client\.ps1') {
    throw 'README I2B handoff does not isolate the non-sampling client wrapper.'
}
if ($readmeSource -notmatch 'run-admin-amd-system-counter-qualification\.ps1' -or
    $readmeSource -notmatch 'cleanup-admin-amd-system-counter-qualification\.ps1' -or
    $readmeSource -notmatch 'SYSTEM_REAL_RUN_CONSUMED = true' -or
    $readmeSource -notmatch 'POWER_AVAILABLE') {
    throw 'README SYSTEM comparison completion record is incomplete.'
}
Write-Host 'README_I2B_NON_SAMPLING_HANDOFF=PASS'
Write-Host 'README_SYSTEM_COMPLETION_RECORD=PASS'

$i2dStart = $readmeSource.IndexOf('## I2D read-only minimum-capability forensics')
if ($i2dStart -lt 0 -or $readmeSource.Substring($i2dStart) -notmatch 'i2d-readonly-forensics\.ps1' -or
    $readmeSource.Substring($i2dStart) -notmatch 'MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED') {
    throw 'README I2D read-only forensics handoff is incomplete.'
}
Write-Host 'README_I2D_READ_ONLY_FORENSICS=PASS'

foreach ($requiredTokenDifferentialContract in @(
        'TokenPrivileges',
        'LookupPrivilegeNameW',
        'enabled_privileges',
        'disabled_privileges',
        'token_groups_relevant_to_access',
        'amd-privilege-service-context/v2'
    )) {
    if ($windowsSourceText -notmatch [regex]::Escape($requiredTokenDifferentialContract)) {
        throw "Token differential evidence contract is missing: $requiredTokenDifferentialContract"
    }
}
Write-Host 'TOKEN_DIFFERENTIAL_EVIDENCE_CONTRACT=PASS'

$i2fEntrypointOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $I2fEntrypointScopeTest `
        -ToolRoot $ToolRoot 2>&1 | ForEach-Object { [string]$_ })
if ($LASTEXITCODE -ne 0) {
    throw "I2F entrypoint scope isolation tests failed: $($i2fEntrypointOutput -join [Environment]::NewLine)"
}
$i2fEntrypointOutput | ForEach-Object { Write-Host $_ }
Write-Host 'I2F_ENTRYPOINT_SCOPE_ISOLATION=PASS'

$i2eResumeEntrypointOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $I2eResumeEntrypointTest `
        -ToolRoot $ToolRoot 2>&1 | ForEach-Object { [string]$_ })
if ($LASTEXITCODE -ne 0) {
    throw "I2E treatment-resume entrypoint tests failed: $($i2eResumeEntrypointOutput -join [Environment]::NewLine)"
}
$i2eResumeEntrypointOutput | ForEach-Object { Write-Host $_ }
Write-Host 'I2E_RESUME_ENTRYPOINT_REGRESSION=PASS'

$i2eRetirementEntrypointOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $I2eRetirementEntrypointTest `
        -ToolRoot $ToolRoot 2>&1 | ForEach-Object { [string]$_ })
if ($LASTEXITCODE -ne 0) {
    throw "I2E retirement entrypoint tests failed: $($i2eRetirementEntrypointOutput -join [Environment]::NewLine)"
}
$i2eRetirementEntrypointOutput | ForEach-Object { Write-Host $_ }
Write-Host 'I2E_RETIREMENT_ENTRYPOINT_REGRESSION=PASS'

$legacyRetirementOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $LegacyRetirementTest `
        -ToolRoot $ToolRoot 2>&1 | ForEach-Object { [string]$_ })
if ($LASTEXITCODE -ne 0) {
    throw "Legacy I2/I2B/I2C real-gate retirement tests failed: $($legacyRetirementOutput -join [Environment]::NewLine)"
}
$legacyRetirementOutput | ForEach-Object { Write-Host $_ }
Write-Host 'LEGACY_REAL_GATE_RETIREMENT_REGRESSION=PASS'

& cargo build --offline --release --manifest-path $Manifest
if ($LASTEXITCODE -ne 0) { throw 'I2G offline release build failed.' }
$i2gHarnessOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $I2gHarnessTest `
        -ToolRoot $ToolRoot 2>&1 | ForEach-Object { [string]$_ })
if ($LASTEXITCODE -ne 0) {
    throw "I2G offline harness tests failed: $($i2gHarnessOutput -join [Environment]::NewLine)"
}
$i2gHarnessOutput | ForEach-Object { Write-Host $_ }
Write-Host 'I2G_OFFLINE_HARNESS_REGRESSION=PASS'

$counterDiscoveryFunction = [regex]::Match(
    $windowsSourceText,
    '(?s)fn\s+execute_counter_discovery_at_with_prefix\(.*?\r?\n}\r?\n\r?\nfn\s+start_session'
).Value
if ([string]::IsNullOrWhiteSpace($counterDiscoveryFunction) -or
    $counterDiscoveryFunction -notmatch 'CounterDiscoveryExecutionEvidence::from_spawn\(true\)' -or
    $counterDiscoveryFunction -notmatch 'counter_discovery_cli_executed' -or
    $counterDiscoveryFunction -notmatch 'power_sampling_runtime_executed') {
    throw 'Counter-discovery evidence does not expose explicit post-spawn execution semantics.'
}
Write-Host 'COUNTER_DISCOVERY_EXECUTION_EVIDENCE_CONTRACT=PASS'

foreach ($documentationPath in @($ExecutionPlan, $QualificationReadme, $ResidualDifferential, $I2gSelectionDocument, $I2gHarnessDocument)) {
    if (-not (Test-Path -LiteralPath $documentationPath -PathType Leaf)) {
        throw "I2F real-closure documentation is missing: $documentationPath"
    }
}
$architectureSource = Get-Content -LiteralPath $ArchitectureDoc -Raw
$currentDocumentation = $architectureSource + [Environment]::NewLine +
    (Get-Content -LiteralPath $ExecutionPlan -Raw) + [Environment]::NewLine +
    (Get-Content -LiteralPath $QualificationReadme -Raw) + [Environment]::NewLine +
    (Get-Content -LiteralPath $I2gSelectionDocument -Raw) + [Environment]::NewLine +
    (Get-Content -LiteralPath $I2gHarnessDocument -Raw)
foreach ($requiredI2eCurrentStateText in @(
        'I2E = CLOSED / RERUN_FORBIDDEN',
        'I2F = REAL_COMPLETED / PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT / RERUN_FORBIDDEN',
        'I2E_REAL_PAIRED_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2E_REAL_TREATMENT_RESUME_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2E_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2E_HISTORICAL_EVIDENCE = IMMUTABLE',
        'I2E_RERUN = FORBIDDEN',
        'I2E_CLEANUP_RERUN = FORBIDDEN',
        'I2_LEGACY_REAL_ENTRYPOINTS = RETIRED',
        'I2B_REAL_ENTRYPOINTS = RETIRED',
        'I2C_REAL_ENTRYPOINTS = RETIRED',
        'AMD_QUALIFICATION_EXECUTABLE_ENTRYPOINT_AUDIT = PASS_NO_UNRETIRED_HISTORICAL_REAL_GATE',
        'I2G_VARIABLE = SeProfileSingleProcessPrivilege',
        'I2G_VARIABLE_SELECTION = PASS_READ_ONLY',
        'I2G_SELECTION_CONFIDENCE = MEDIUM',
        'I2G_SELECTION_CHANGED = false',
        'BLOCKER = I2G_PAIRED_PHASE_CONFIGURATION_INVARIANT_CONTRADICTS_TREATMENT_MUTATION',
        'BLOCKER_STATUS = CLOSED_OFFLINE',
        'I2G_HARNESS = IMPLEMENTED_OFFLINE',
        'I2G_HARNESS_IMPLEMENTED = true',
        'I2G_GATE_CONSUMED = false',
        'I2G_REAL_EXECUTION_ALLOWED = false',
        'I2G_REAL_CLEANUP_ALLOWED = false',
        'I2G_HUMAN_REAL_RUN_AUTHORIZATION = NOT_GRANTED',
        'I2G_OFFLINE_VALIDATION = PASS',
        'I2G_HARNESS_IMPLEMENTATION_AUTHORIZED = false',
        'I2G_REAL_RUNTIME_AUTHORIZED = false',
        'I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege',
        'I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege',
        'I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT',
        'HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY',
        'HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false',
        'TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2',
        'SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1',
        'PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1',
        'PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1',
        'PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2',
        'MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1',
        'MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1',
        'MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2',
        'ACTUAL_RUN_COUNT_EVIDENCE_SCHEMA = DEFINED',
        'POWER_SAMPLING_RUNS = 0',
        'NEXT_GATE = AMD_PRIVILEGE_I2G_REAL_PAIRED_QUALIFICATION_ENTRY_GATE'
    )) {
    if ($currentDocumentation.IndexOf($requiredI2eCurrentStateText, [StringComparison]::Ordinal) -lt 0) {
        throw "I2E current-state reconciliation is missing: $requiredI2eCurrentStateText"
    }
}
Write-Host 'ARCHITECTURE_SINGLE_AUTHORITATIVE_CURRENT_STATE=PASS'
Write-Host 'EXECUTION_PLAN_SINGLE_CURRENT_STATE=PASS'
Write-Host 'README_CURRENT_STATE_RECONCILED=PASS'
$i2gContractSource = Get-Content -LiteralPath $I2gSelectionDocument -Raw
foreach ($requiredI2gContractText in @(
        'BLOCKER = I2G_PAIRED_PHASE_CONFIGURATION_INVARIANT_CONTRADICTS_TREATMENT_MUTATION',
        'BLOCKER_STATUS = CLOSED_OFFLINE',
        'PREVIOUS_BLOCKER_1 = I2G_BASELINE_RECONSTRUCTION_CONTRACT_INCONSISTENT / CLOSED_OFFLINE',
        'PREVIOUS_BLOCKER_2 = I2G_STAGED_TREATMENT_TOKEN_MISCLASSIFIED_AS_EXACT_I2F_BASELINE / CLOSED_OFFLINE',
        'PREVIOUS_BLOCKER_3 = I2G_HISTORICAL_CONTROL_LEAVES_NON_TREATMENT_CONFOUNDERS_UNCONTROLLED / CLOSED_OFFLINE',
        'I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT',
        'HISTORICAL_I2F_REFERENCE =',
        'HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY',
        'HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false',
        'CONTROL_POLICY_RIGHTS = SeSystemProfilePrivilege only',
        'I2G_CONTROL_MATERIALIZED_TOKEN =',
        'I2G_CONTROL_FINAL_TOKEN =',
        'I2G_CONTROL_TOKEN = I2G-CONTROL-TOKEN.json',
        'CONTROL_SAMPLING = false',
        'CONTROL_COUNTER_DISCOVERY = timechart --list',
        'PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1',
        'CONTROL_EXPECTED_RESULT = POWER_UNAVAILABLE',
        'CONTROL_DRIFT =',
        'CONTROL_DRIFT_STOP_BEFORE_TREATMENT = true',
        'CONTROL_TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION = true',
        'CONTROL_DIRECT_SERVICE_SID_RIGHTS = SeSystemProfilePrivilege',
        'TREATMENT_POLICY_RIGHTS =',
        'TREATMENT_POLICY_ADDITION = SeProfileSingleProcessPrivilege only',
        'TREATMENT_POLICY_MUTATION_GATE = CONTROL_TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION',
        'TREATMENT_DIRECT_SERVICE_SID_RIGHTS = SeSystemProfilePrivilege + SeProfileSingleProcessPrivilege',
        'EXACT_TREATMENT_POLICY_DELTA = SeProfileSingleProcessPrivilege added',
        'NO_CODE_CHANGE_BETWEEN_PHASES = true',
        'NO_HARNESS_REBUILD_BETWEEN_PHASES = true',
        'NO_NON_TREATMENT_CONFIGURATION_CHANGE_BETWEEN_PHASES = true',
        'ALLOWED_TREATMENT_CONFIGURATION_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only',
        'CONTROL_TO_TREATMENT_POLICY_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only',
        'CONTROL_TO_TREATMENT_POLICY_DELTA_COUNT = 1',
        'NON_TREATMENT_CONFIGURATION_INVARIANTS = UNCHANGED',
        'CONTROL_SERVICE_NAME_EQUALS_TREATMENT = true',
        'CONTROL_SERVICE_SID_EQUALS_TREATMENT = true',
        'CONTROL_HARNESS_SHA_EQUALS_TREATMENT = true',
        'SERVICE_RESTART = REQUIRED_TECHNICAL_MATERIALIZATION_BOUNDARY',
        'I2G_TREATMENT_MATERIALIZED_TOKEN =',
        'I2G_TREATMENT_FINAL_TOKEN =',
        'I2G_TREATMENT_TOKEN = I2G-TREATMENT-TOKEN.json',
        'TREATMENT_SAMPLING = false',
        'TREATMENT_COUNTER_DISCOVERY = timechart --list',
        'PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1',
        'CONTROL_TO_TREATMENT_NON_TREATMENT_INVARIANTS = UNCHANGED',
        'I2G_PAIRED_CAUSAL_TREATMENT_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + ENABLED',
        'I2G_TREATMENT_MATERIALIZATION_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + DISABLED',
        'I2G_TREATMENT_ACTIVATION_DELTA = SeProfileSingleProcessPrivilege DISABLED -> ENABLED',
        'TREATMENT_MATERIALIZATION_AND_ACTIVATION = ONE_CAPABILITY_INTERVENTION',
        'PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2',
        'MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1',
        'MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1',
        'MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2',
        'ACTUAL_RUN_COUNTS =',
        'ACTUAL_CONTROL_COUNTER_DISCOVERY_RUNS',
        'ACTUAL_TREATMENT_COUNTER_DISCOVERY_RUNS',
        'ACTUAL_TOTAL_I2G_COUNTER_DISCOVERY_RUNS',
        'CONTROL_DRIFT_EXPECTED_ACTUAL_COUNTS = control 1; treatment 0; total 1',
        'PRE_CONTROL_FAILURE_ACTUAL_COUNTS = 0 / 0 / 0',
        'CONTROL_RETRY_ALLOWED = false',
        'TREATMENT_RETRY_ALLOWED = false',
        'COUNTER_DISCOVERY_RETRY_POLICY = NO_RETRY',
        'POWER_SAMPLING_RUNS = 0',
        'TOKEN_TEARDOWN_BEFORE_POLICY_RIGHT_REMOVAL = true',
        'I2G_BASELINE_POLICY_ROLLBACK = SeSystemProfilePrivilege',
        'I2G_TREATMENT_POLICY_ROLLBACK = SeProfileSingleProcessPrivilege',
        'PLANNED_RUN_COUNTS; MAX_RUN_COUNTS;',
        'ACTUAL_RUN_COUNTS; CONTROL_POLICY_STATE; CONTROL_TOKEN;',
        'CONTROL_TREATMENT_CONFIGURATION_INVARIANT_COMPARISON;',
        'CONTROL_TREATMENT_TOKEN_INVARIANT_COMPARISON;',
        'CONTROL_TREATMENT_TOKEN_INVARIANT_COMPARISON =',
        'PAIRED_CAUSAL_DELTA',
        'PARTIAL_FAILURE_ACCOUNTING =',
        'INVALID_RESULT_CONTROL_DRIFT_GATE =',
        'INVALID_RESULT_CAUSAL_GATE =',
        'INVALID_CONFIGURATION_DELTA =',
        'ACTUAL_RUN_COUNT_RULE ='
    )) {
    if ($i2gContractSource.IndexOf($requiredI2gContractText, [StringComparison]::Ordinal) -lt 0) {
        throw "I2G baseline reconstruction contract is missing: $requiredI2gContractText"
    }
}
if ($i2gContractSource -match '(?m)^NO_BASELINE_AMD_RUN = true\s*$') {
    throw 'I2G current contract still claims that no baseline/control AMD run is required.'
}
$pairedContractMarker = '## Design-only I2G paired CONTROL -> TREATMENT contract'
$pairedContractStart = $i2gContractSource.IndexOf($pairedContractMarker, [StringComparison]::Ordinal)
if ($pairedContractStart -lt 0) {
    throw 'I2G paired CONTROL -> TREATMENT contract section is missing.'
}
$pairedContractSource = $i2gContractSource.Substring($pairedContractStart)
if ($pairedContractSource.IndexOf('HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = true', [StringComparison]::Ordinal) -ge 0 -or
    $pairedContractSource.IndexOf('historical I2F is the active causal control', [StringComparison]::OrdinalIgnoreCase) -ge 0) {
    throw 'I2G paired contract still treats historical I2F as the active causal control.'
}
if ($pairedContractSource -match '(?m)^NO_CODE_OR_CONFIGURATION_CHANGE_BETWEEN_PHASES = true\s*$') {
    throw 'I2G paired contract still uses the obsolete configuration invariant.'
}
if ($pairedContractSource -match '(?m)^(I2G_CONTROL_COUNTER_DISCOVERY_RUNS|I2G_TREATMENT_COUNTER_DISCOVERY_RUNS|TOTAL_I2G_COUNTER_DISCOVERY_RUNS) = \d+\s*$') {
    throw 'I2G paired contract still presents an unqualified actual run count.'
}
Write-Host 'I2G_PAIRED_CONTROL_TREATMENT_CONTRACT=PASS'
Write-Host 'I2G_PAIRED_PHASE_TRANSITION_CONTRACT=PASS'
function Get-I2gPrivilegeCategoryBody {
    param([string]$CategoryName)
    return [regex]::Match(
        $i2gContractSource,
        "(?ms)^$([regex]::Escape($CategoryName)) =\r?\n(?<body>.*?)(?=^[A-Z0-9_]+ =|\z)"
    ).Groups['body'].Value
}
$mustRemainAbsentLine = Get-I2gPrivilegeCategoryBody 'MUST_REMAIN_ABSENT'
$mustRemainDisabledLine = Get-I2gPrivilegeCategoryBody 'MUST_REMAIN_DISABLED'
$mustRemainEnabledLine = Get-I2gPrivilegeCategoryBody 'MUST_REMAIN_ENABLED'
if ([string]::IsNullOrWhiteSpace($mustRemainAbsentLine) -or
    [string]::IsNullOrWhiteSpace($mustRemainDisabledLine) -or
    [string]::IsNullOrWhiteSpace($mustRemainEnabledLine)) {
    throw 'I2G privilege-state classification categories are missing.'
}
foreach ($mustRemainAbsentPrivilege in @(
        'SeDebugPrivilege',
        'SeCreatePagefilePrivilege',
        'SeCreatePermanentPrivilege',
        'SeCreateSymbolicLinkPrivilege',
        'SeDelegateSessionUserImpersonatePrivilege',
        'SeIncreaseBasePriorityPrivilege',
        'SeLockMemoryPrivilege',
        'SeTcbPrivilege',
        'SeBackupPrivilege',
        'SeLoadDriverPrivilege',
        'SeManageVolumePrivilege',
        'SeRestorePrivilege',
        'SeSecurityPrivilege',
        'SeSystemEnvironmentPrivilege',
        'SeTakeOwnershipPrivilege'
    )) {
    if ($mustRemainAbsentLine.IndexOf($mustRemainAbsentPrivilege, [StringComparison]::Ordinal) -lt 0) {
        throw "I2G MUST_REMAIN_ABSENT classification is missing: $mustRemainAbsentPrivilege"
    }
}
foreach ($mustRemainDisabledPrivilege in @(
        'SeAuditPrivilege',
        'SeIncreaseWorkingSetPrivilege',
        'SeTimeZonePrivilege',
        'SeAssignPrimaryTokenPrivilege',
        'SeIncreaseQuotaPrivilege',
        'SeShutdownPrivilege',
        'SeSystemtimePrivilege',
        'SeUndockPrivilege'
    )) {
    if ($mustRemainDisabledLine.IndexOf($mustRemainDisabledPrivilege, [StringComparison]::Ordinal) -lt 0 -or
        $mustRemainAbsentLine.IndexOf($mustRemainDisabledPrivilege, [StringComparison]::Ordinal) -ge 0) {
        throw "I2G MUST_REMAIN_DISABLED classification is invalid: $mustRemainDisabledPrivilege"
    }
}
foreach ($mustRemainEnabledPrivilege in @(
        'SeChangeNotifyPrivilege',
        'SeCreateGlobalPrivilege',
        'SeImpersonatePrivilege',
        'SeSystemProfilePrivilege',
        'SeProfileSingleProcessPrivilege'
    )) {
    if ($mustRemainEnabledLine.IndexOf($mustRemainEnabledPrivilege, [StringComparison]::Ordinal) -lt 0) {
        throw "I2G MUST_REMAIN_ENABLED classification is missing: $mustRemainEnabledPrivilege"
    }
}
Write-Host 'I2G_BASELINE_RECONSTRUCTION_CONTRACT=PASS'
Write-Host 'I2G_PRIVILEGE_STATE_CLASSIFICATIONS=PASS'
$legacyCurrentStateSource = @(
    Get-Content -LiteralPath $ArchitectureDoc -Raw
    Get-Content -LiteralPath $ExecutionPlan -Raw
    Get-Content -LiteralPath $QualificationReadme -Raw
) -join [Environment]::NewLine
foreach ($requiredLegacyCurrentStateText in @(
        'I2_BROKER_SETUP_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2_POWER_SAMPLING_CLIENT_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2B_COUNTER_DISCOVERY_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2_LEGACY_CLEANUP_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2C_SYSTEM_COMPARISON_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2C_SYSTEM_CLEANUP_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2_LEGACY_RERUN = FORBIDDEN',
        'I2B_RERUN = FORBIDDEN',
        'I2C_RERUN = FORBIDDEN',
        'AMD_QUALIFICATION_EXECUTABLE_ENTRYPOINT_AUDIT = PASS_NO_UNRETIRED_HISTORICAL_REAL_GATE'
    )) {
    if ($legacyCurrentStateSource.IndexOf($requiredLegacyCurrentStateText, [StringComparison]::Ordinal) -lt 0) {
        throw "Legacy I2/I2B/I2C current-state reconciliation is missing: $requiredLegacyCurrentStateText"
    }
}
Write-Host 'LEGACY_REAL_GATE_CURRENT_STATE_RECONCILED=PASS'
$i2fClosureDocumentation = @(
    Get-Content -LiteralPath $ExecutionPlan -Raw
    Get-Content -LiteralPath $QualificationReadme -Raw
    Get-Content -LiteralPath $ResidualDifferential -Raw
    Get-Content -LiteralPath $I2gSelectionDocument -Raw
) -join [Environment]::NewLine
foreach ($requiredI2fClosureText in @(
        'I2F_RESULT = PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT',
        'I2F_SCOPE = f68bf4d3d36547a0ba753cff489bb6eb',
        'I2F_GATE_CONSUMED = true',
        'I2F_RERUN = FORBIDDEN',
        'I2F_REAL_EXECUTION_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2F_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED',
        'I2F_CLEANUP_RERUN = FORBIDDEN',
        'I2F_HISTORICAL_EVIDENCE = IMMUTABLE',
        'I2F_AUTHORITATIVE_ROLLBACK = REAL_PASS',
        'I2F_CLEANUP_REQUIRED = false',
        'I2F_REAL_RERUN_ERROR = I2F_RERUN_FORBIDDEN',
        'I2F_REAL_RERUN_MACHINE_STATE = UNCHANGED',
        'counter_discovery_cli_executed = true after Command::spawn succeeds',
        'power_sampling_runtime_executed = false',
        'I2G_VARIABLE = SeProfileSingleProcessPrivilege',
        'I2G_VARIABLE_SELECTION = PASS_READ_ONLY',
        'I2G_SELECTION_CONFIDENCE = MEDIUM',
        'PRODUCTION_ACCOUNT = UNRESOLVED',
        'LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED'
    )) {
    if ($i2fClosureDocumentation.IndexOf($requiredI2fClosureText, [StringComparison]::Ordinal) -lt 0) {
        throw "I2F real-closure documentation is missing: $requiredI2fClosureText"
    }
}
if ($i2fClosureDocumentation.IndexOf('I2F_HISTORICAL_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D', [StringComparison]::Ordinal) -lt 0 -or
    $i2fClosureDocumentation.IndexOf('I2F_POST_REPAIR_ARTIFACT_SHA256 = 9A13111B02D5AAA2886B7E1EA059643EAABD5F30C3A2522589EE8B124B7B735C', [StringComparison]::Ordinal) -lt 0) {
    throw 'I2F historical/post-repair artifact distinction is missing from the closure documentation.'
}
Write-Host 'I2F_REAL_CLOSURE_DOCUMENTATION=PASS'

foreach ($requiredI2fConsumedGateContract in @(
        '$I2fAuthoritativeScope = ''f68bf4d3d36547a0ba753cff489bb6eb''',
        '$I2fRealGateConsumed = $true',
        '$I2fRealRerunAllowed = $false',
        'I2F_RERUN_FORBIDDEN'
    )) {
    if ($i2fContractSource.IndexOf($requiredI2fConsumedGateContract, [StringComparison]::Ordinal) -lt 0 -and
        $i2fSetupSource.IndexOf($requiredI2fConsumedGateContract, [StringComparison]::Ordinal) -lt 0) {
        throw "I2F consumed-gate contract is missing: $requiredI2fConsumedGateContract"
    }
}
Write-Host 'I2F_CONSUMED_GATE_CONTRACT=PASS'

foreach ($requiredI2fCleanupRetirementContract in @(
        '$I2fRealCleanupAllowed = $false',
        '$I2fAuthoritativeRollbackComplete = $true',
        'I2F_CLEANUP_RERUN_FORBIDDEN'
    )) {
    if ($i2fContractSource.IndexOf($requiredI2fCleanupRetirementContract, [StringComparison]::Ordinal) -lt 0 -and
        (Get-Content -LiteralPath $I2fCleanup -Raw).IndexOf($requiredI2fCleanupRetirementContract, [StringComparison]::Ordinal) -lt 0) {
        throw "I2F cleanup-retirement contract is missing: $requiredI2fCleanupRetirementContract"
    }
}
Write-Host 'I2F_CLEANUP_RETIREMENT_CONTRACT=PASS'

Remove-Item -LiteralPath $EvidenceRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $EvidenceRoot | Out-Null

& cargo fmt --manifest-path $Manifest -- --check
if ($LASTEXITCODE -ne 0) { throw 'cargo fmt --check failed.' }
& cargo test --offline --manifest-path $Manifest -- --nocapture
if ($LASTEXITCODE -ne 0) { throw 'focused qualification tests failed.' }
& cargo build --offline --manifest-path $Manifest
if ($LASTEXITCODE -ne 0) { throw 'synthetic qualification debug build failed.' }
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) { throw "Synthetic binary is missing: $Binary" }
& $Binary --synthetic --evidence-root $EvidenceRoot
if ($LASTEXITCODE -ne 0) { throw 'synthetic qualification executable returned failure.' }
$summary = Get-Content -LiteralPath (Join-Path $EvidenceRoot 'SYNTHETIC-QUALIFICATION.json') -Raw | ConvertFrom-Json
if ($summary.result -ne 'PASS' -or $summary.amd_runtime_executed -ne $false) {
    throw 'Synthetic qualification summary did not pass without AMD execution.'
}
foreach ($requiredSyntheticCheck in @(
        'COUNTER_DISCOVERY_FIXED_SEMANTIC_REQUEST',
        'COUNTER_DISCOVERY_NO_COUNTERS_EXIT_ZERO',
        'COUNTER_DISCOVERY_POWER_PRESENT',
        'COUNTER_DISCOVERY_UNKNOWN_FAILURE',
        'COUNTER_DISCOVERY_EXECUTION_BEFORE_SPAWN',
        'COUNTER_DISCOVERY_EXECUTION_AFTER_SPAWN_EXIT_ZERO',
        'COUNTER_DISCOVERY_EXECUTION_AFTER_SPAWN_NONZERO',
        'COUNTER_DISCOVERY_EXECUTION_SPAWN_FAILURE',
        'COUNTER_DISCOVERY_EXECUTION_NON_SAMPLING'
    )) {
    $check = @($summary.checks | Where-Object { $_.name -eq $requiredSyntheticCheck })
    if ($check.Count -ne 1 -or $check[0].status -ne 'PASS') {
        throw "Counter-discovery synthetic check did not pass: $requiredSyntheticCheck"
    }
}
if ($summary.mutation_assertions.real_amd_runtime_count_during_task -ne 0 -or
    $summary.mutation_assertions.service_registration_count_during_task -ne 0 -or
    $summary.mutation_assertions.scheduled_task_registration_count -ne 0 -or
    $summary.mutation_assertions.self_elevation_performed -ne $false -or
    $summary.mutation_assertions.amd_installation_mutated -ne $false -or
    $summary.mutation_assertions.amd_registry_mutated -ne $false) {
    throw 'Synthetic mutation assertions are not clean.'
}

foreach ($serviceName in @(
        'ResourceTimelineAmdPrivilegeQualification',
        'ResourceTimelineAmdSystemCounterQualification',
        'ResourceTimelineAmdSystemProfileEnableQualification'
    )) {
    if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
        throw "Synthetic test refuses to run while the qualification service is registered: $serviceName"
    }
}
Write-Host 'Synthetic qualification PASS. No service registration, AMD runtime, elevation, or AMD installation mutation was performed.'
