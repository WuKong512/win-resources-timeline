[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ToolRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$RunnerPath = Join-Path $ToolRoot 'run-amd-localservice-active-sampling.ps1'
$ServiceHostPath = Join-Path $ToolRoot 'service-host.ps1'
. (Join-Path $ToolRoot 'contract.ps1')
. (Join-Path $ToolRoot '..\amd-uprof-cli-spike\postprocess.ps1')
. (Join-Path $ToolRoot '..\amd-privilege-qualification\sc-argument-contract.ps1')

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        throw "ASSERTION_FAILED: $Message"
    }
}

function Assert-Equal {
    param(
        [Parameter(Mandatory = $true)]$Actual,
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if ([string]$Actual -cne [string]$Expected) {
        throw "ASSERTION_FAILED: $Message; expected=$Expected actual=$Actual"
    }
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Needle,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if ($Text.IndexOf($Needle, [StringComparison]::Ordinal) -lt 0) {
        throw "ASSERTION_FAILED: $Message; missing=$Needle"
    }
}

function Assert-ArrayEqual {
    param(
        [Parameter(Mandatory = $true)][object[]]$Actual,
        [Parameter(Mandatory = $true)][object[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Message
    )
    $actualText = @($Actual | ForEach-Object { [string]$_ }) -join [char]0
    $expectedText = @($Expected | ForEach-Object { [string]$_ }) -join [char]0
    Assert-Equal -Actual $actualText -Expected $expectedText -Message $Message
}

function Test-PowerShellSyntax {
    param([Parameter(Mandatory = $true)][string]$Path)

    $tokens = $null
    $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors) | Out-Null
    Assert-Equal -Actual $errors.Count -Expected 0 -Message "PowerShell syntax: $Path"
}

function New-TestRoot {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('amd-localservice-active-sampling-test-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $root | Out-Null
    $root
}

$contract = Get-AmdLocalServiceSamplingContract
Assert-Equal -Actual $contract.task_id -Expected 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1' -Message 'task id'
Assert-Equal -Actual $contract.account_sid -Expected 'S-1-5-19' -Message 'LocalService SID'
Assert-Equal -Actual $contract.service_sid_type -Expected 'unrestricted' -Message 'Service SID type'
Assert-Equal -Actual $contract.session_id -Expected 0 -Message 'Session 0'
Assert-Equal -Actual $contract.interactive -Expected $false -Message 'non-interactive contract'
Assert-Equal -Actual $contract.amd_cli_sha256 -Expected 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC' -Message 'CLI SHA256'
Assert-Equal -Actual $contract.amd_cli_file_version -Expected '5.3.521.0' -Message 'CLI file version'
Assert-Equal -Actual $contract.amd_cli_interval_ms -Expected 1000 -Message 'CLI interval'
Assert-Equal -Actual $contract.amd_cli_duration_seconds -Expected 10 -Message 'CLI duration'
Assert-Equal -Actual $contract.max_runs -Expected 1 -Message 'one-shot max runs'
Assert-Equal -Actual $contract.retries -Expected 0 -Message 'retry prohibition'
Assert-Equal -Actual $contract.authorization_token -Expected 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1-REVIEWED-PREFLIGHT-FIX-I1' -Message 'new reviewed-head authorization token'
Assert-Equal -Actual $contract.authorization_environment_value -Expected 'GRANTED_FOR_NEW_REVIEWED_HEAD_ONLY' -Message 'new reviewed-head authorization marker'
Assert-True -Condition $contract.live_service_mutation_supported -Message 'live service mutation capability is explicit'
Assert-True -Condition $contract.live_output_acl_mutation_supported -Message 'live output ACL capability is explicit'
Assert-True -Condition $contract.live_control_baseline_lsa_mutation_supported -Message 'live CONTROL baseline LSA capability is explicit'
Assert-Equal -Actual $contract.allowed_lsa_right -Expected 'SeSystemProfilePrivilege' -Message 'allowed LSA right'
Assert-Equal -Actual $contract.allowed_lsa_target -Expected 'EXACT_Q1_SERVICE_SID_ONLY' -Message 'allowed LSA target'

$command = @(Get-FixedAmdCliArguments -OutputDirectory 'C:\ProgramData\run\raw\timechart-output')
Assert-Equal -Actual ($command -join '|') -Expected 'timechart|--event|power|--interval|1000|--duration|10|--format|csv|--output-dir|C:\ProgramData\run\raw\timechart-output' -Message 'exact CLI command'
Assert-True -Condition (-not ($command -contains '--list')) -Message 'active command must not include discovery'
Assert-True -Condition (-not ($command -contains '--temperature')) -Message 'active command must not add temperature'
Assert-True -Condition (-not ($command -contains '--frequency')) -Message 'active command must not add frequency'

$serviceBinPath = 'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoLogo -File C:\qualification\service-host.ps1 -ManifestPath C:\qualification\manifest.json'
$serviceArgs = @(New-QualificationServiceCreateArguments -ServiceName $contract.service_name -BinPath $serviceBinPath -ServiceAccount $contract.account -DisplayName $contract.service_display_name)
$expectedServiceArgs = @(
    'create'
    $contract.service_name
    'binPath='
    $serviceBinPath
    'start='
    'demand'
    'obj='
    $contract.account
    'type='
    'own'
    'DisplayName='
    $contract.service_display_name
)
Assert-ArrayEqual -Actual $serviceArgs -Expected $expectedServiceArgs -Message 'exact sc.exe create argv contract'
Assert-Equal -Actual $serviceArgs[1] -Expected $contract.service_name -Message 'service name argv'
Assert-Equal -Actual $serviceArgs[3] -Expected $serviceBinPath -Message 'service binPath argv'
Assert-Equal -Actual $serviceArgs[7] -Expected $contract.account -Message 'LocalService account argv'
Assert-Equal -Actual $serviceArgs[5] -Expected 'demand' -Message 'manual start argv'
Assert-Equal -Actual $serviceArgs[9] -Expected 'own' -Message 'own-process argv'
Assert-Equal -Actual $serviceArgs[11] -Expected $contract.service_display_name -Message 'display name argv'
$serviceConfigurationFixture = [pscustomobject]@{
    present = $true
    name = $contract.service_name
    start_name = $contract.account
    start_mode = 'Manual'
    service_type = 'Own Process'
    display_name = $contract.service_display_name
    path_name = $serviceBinPath
}
$serviceSidFixture = [pscustomobject]@{
    valid = $true
    sid = 'S-1-5-80-1-2-3-4-5'
    sid_type = 'unrestricted'
}
$serviceConfigurationGate = Test-Q1ServiceConfigurationEvidence -Evidence $serviceConfigurationFixture -Contract $contract -ExpectedBinPath $serviceBinPath -ServiceSidEvidence $serviceSidFixture
Assert-True -Condition $serviceConfigurationGate.valid -Message 'service configuration fixture validates all frozen fields'
$badDisplayFixture = $serviceConfigurationFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$badDisplayFixture.display_name = 'unexpected display name'
$badDisplayGate = Test-Q1ServiceConfigurationEvidence -Evidence $badDisplayFixture -Contract $contract -ExpectedBinPath $serviceBinPath -ServiceSidEvidence $serviceSidFixture
Assert-True -Condition (-not $badDisplayGate.valid) -Message 'service display-name drift must block'

$tokenFixture = Get-OfflineTokenFixture
$expectedServiceSid = 'S-1-5-80-1234567890-1234567890-1234567890-1234567890-1234'
$tokenGate = Test-EffectiveTokenEvidence -Evidence $tokenFixture -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition $tokenGate.valid -Message 'offline CONTROL token fixture'
$wrongServiceSidGate = Test-EffectiveTokenEvidence -Evidence $tokenFixture -Contract $contract -ExpectedServiceSid 'S-1-5-80-1-2-3-4-5'
Assert-True -Condition (-not $wrongServiceSidGate.valid) -Message 'unexpected Service SID must be rejected'
$profileSingle = $tokenFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$profileSingle.privileges += [pscustomobject]@{ name = 'SeProfileSingleProcessPrivilege'; state = 'Enabled' }
$profileGate = Test-EffectiveTokenEvidence -Evidence $profileSingle -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition (-not $profileGate.valid) -Message 'ProfileSingle must be rejected'
$adminGroup = $tokenFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$adminGroup.group_sids += 'S-1-5-32-544'
$adminGate = Test-EffectiveTokenEvidence -Evidence $adminGroup -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition (-not $adminGate.valid) -Message 'Administrators membership must be rejected'
$missingSystemProfile = $tokenFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$missingSystemProfile.privileges = @($missingSystemProfile.privileges |
    Where-Object { $_.name -cne 'SeSystemProfilePrivilege' })
$missingSystemProfileGate = Test-EffectiveTokenEvidence -Evidence $missingSystemProfile -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition (-not $missingSystemProfileGate.valid) -Message 'missing SystemProfile must be rejected'

$binary = Get-OfflineBinaryFixture -Contract $contract
$binaryGate = Test-BinaryIdentityEvidence -Identity $binary -Contract $contract
Assert-True -Condition $binaryGate.valid -Message 'offline binary identity fixture'
$badBinary = $binary | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$badBinary.sha256 = ('0' * 64)
$badBinaryGate = Test-BinaryIdentityEvidence -Identity $badBinary -Contract $contract
Assert-True -Condition (-not $badBinaryGate.valid) -Message 'binary SHA mismatch must block'

$drivers = @(Get-OfflineDriverFixture -Contract $contract)
$driverGate = Test-DriverVersionEvidence -Drivers $drivers -Contract $contract
Assert-True -Condition $driverGate.valid -Message 'offline driver identity fixtures'
$badDrivers = @($drivers | ForEach-Object {
    $copy = $_ | ConvertTo-Json -Depth 10 | ConvertFrom-Json
    if ($copy.name -eq 'AMDPowerProfiler') { $copy.file_version = '0.0.0.0' }
    $copy
})
$badDriverGate = Test-DriverVersionEvidence -Drivers $badDrivers -Contract $contract
Assert-True -Condition (-not $badDriverGate.valid) -Message 'driver version mismatch must block'
$driverContract = Get-Q1DriverContractValidation -Contract (Get-AmdLocalServiceSamplingContract)
Assert-True -Condition $driverContract.valid -Message 'frozen driver contract validates'
Assert-Equal -Actual $driverContract.entries.Count -Expected 2 -Message 'exactly two frozen driver entries are enumerated'
Assert-ArrayEqual -Actual $driverContract.actual_keys -Expected @('AMDPowerProfiler', 'AMDCpuProfiler') -Message 'OrderedDictionary driver keys'
Assert-True -Condition (-not (@($driverContract.actual_keys | Where-Object { $_ -in @('Count', 'Keys', 'Values', 'SyncRoot') }).Count -gt 0)) -Message 'OrderedDictionary adapter properties are excluded'
Assert-True -Condition ((Get-AmdLocalServiceSamplingContract).driver_versions -is [System.Collections.Specialized.OrderedDictionary]) -Message 'driver contract is an OrderedDictionary'
$hostDriverProbe = {
    param($Name, $Expected)
    [pscustomobject]@{
        name = $Name
        path = Get-Q1DictionaryValue -Object $Expected -Name 'path'
        exists = $true
        file_version = Get-Q1DictionaryValue -Object $Expected -Name 'file_version'
        product_version = Get-Q1DictionaryValue -Object $Expected -Name 'product_version'
        sha256 = ('A' * 64)
        signature_status = 'Valid'
        signer = 'CN=Advanced Micro Devices, Inc.'
        service_state = 'Running'
    }
}
$hostDrivers = @(Get-HostDriverEvidence -Contract $contract -Probe $hostDriverProbe)
Assert-Equal -Actual $hostDrivers.Count -Expected 2 -Message 'host driver evidence returns both frozen drivers'
Assert-True -Condition (-not (@($hostDrivers | Where-Object { $_.name -in @('Count', 'Keys', 'Values', 'SyncRoot') }).Count -gt 0)) -Message 'host evidence does not emit adapter properties as drivers'
Assert-True -Condition (@($hostDrivers | Where-Object { $_.name -eq 'AMDPowerProfiler' -and $_.exists -and $_.service_state -eq 'Running' }).Count -eq 1) -Message 'power driver host evidence is structured'
Assert-True -Condition (@($hostDrivers | Where-Object { $_.name -eq 'AMDCpuProfiler' -and $_.exists -and $_.service_state -eq 'Running' }).Count -eq 1) -Message 'CPU driver host evidence is structured'
$missingDriverProbe = {
    param($Name, $Expected)
    [pscustomobject]@{
        name = $Name
        path = Get-Q1DictionaryValue -Object $Expected -Name 'path'
        exists = $false
        service_state = $null
        error = 'offline missing-driver fixture'
    }
}
$missingDriverEvidence = @(Get-HostDriverEvidence -Contract $contract -Probe $missingDriverProbe)
Assert-True -Condition (@($missingDriverEvidence | Where-Object { -not $_.exists -and $_.failure_categories -contains 'HOST_DRIVER_MISSING' -and $_.error }).Count -eq 2) -Message 'missing driver returns structured failure records'
$missingDriverGate = Test-DriverVersionEvidence -Drivers $missingDriverEvidence -Contract $contract
Assert-True -Condition (-not $missingDriverGate.valid) -Message 'missing driver fails the preflight driver gate cleanly'
$mismatchDriverProbe = {
    param($Name, $Expected)
    $fileVersion = if ($Name -eq 'AMDPowerProfiler') { '0.0.0.0' } else { Get-Q1DictionaryValue -Object $Expected -Name 'file_version' }
    [pscustomobject]@{
        name = $Name
        path = Get-Q1DictionaryValue -Object $Expected -Name 'path'
        exists = $true
        file_version = $fileVersion
        product_version = Get-Q1DictionaryValue -Object $Expected -Name 'product_version'
        sha256 = ('B' * 64)
        signature_status = 'Valid'
        signer = 'CN=Advanced Micro Devices, Inc.'
        service_state = 'Running'
    }
}
$mismatchDriverEvidence = @(Get-HostDriverEvidence -Contract $contract -Probe $mismatchDriverProbe)
$mismatchDriverGate = Test-DriverVersionEvidence -Drivers $mismatchDriverEvidence -Contract $contract
Assert-True -Condition (-not $mismatchDriverGate.valid) -Message 'driver version mismatch fails the preflight driver gate'
$queryFailureDriverEvidence = @(Get-HostDriverEvidence -Contract $contract -Probe { param($Name, $Expected) throw "offline host query failure: $Name" })
Assert-True -Condition (@($queryFailureDriverEvidence | Where-Object { $_.failure_category -eq 'HOST_DRIVER_QUERY_FAILURE' -and -not [string]::IsNullOrWhiteSpace([string]$_.error) }).Count -eq 2) -Message 'driver query exception is structured and fail closed'
$malformedDriverContract = Get-AmdLocalServiceSamplingContract
$malformedDriverContract.driver_versions.AMDPowerProfiler.path = $null
$malformedDriverValidation = Get-Q1DriverContractValidation -Contract $malformedDriverContract
Assert-True -Condition (-not $malformedDriverValidation.valid) -Message 'malformed driver contract is rejected'
$malformedDriverEvidence = @(Get-HostDriverEvidence -Contract $malformedDriverContract -Probe $hostDriverProbe)
Assert-True -Condition (@($malformedDriverEvidence | Where-Object { $_.name -eq 'AMDPowerProfiler' -and -not $_.contract_valid -and $_.failure_category -eq 'CONTRACT_ENUMERATION_FAILURE' }).Count -eq 1) -Message 'malformed driver contract returns structured evidence'
$noisyDriverContract = Get-AmdLocalServiceSamplingContract
[void]$noisyDriverContract.driver_versions.Add('Count', [ordered]@{ path = 'C:\noise.sys'; file_version = '0'; product_version = '0'; signer_pattern = 'noise'; file_name = 'noise.sys' })
$noisyDriverValidation = Get-Q1DriverContractValidation -Contract $noisyDriverContract
Assert-True -Condition (-not $noisyDriverValidation.valid) -Message 'unexpected driver contract key is rejected'
$noisyDriverEvidence = @(Get-HostDriverEvidence -Contract $noisyDriverContract -Probe $hostDriverProbe)
Assert-Equal -Actual $noisyDriverEvidence.Count -Expected 2 -Message 'unexpected driver contract key cannot become a host driver record'
$sourceIdentity = Test-HarnessSourceIdentity -Root $ToolRoot -Contract $contract
Assert-True -Condition $sourceIdentity.valid -Message 'reviewed harness source identity'
$driftContract = Get-AmdLocalServiceSamplingContract
$driftContract.harness_source_sha256.runner = ('0' * 64)
$driftIdentity = Test-HarnessSourceIdentity -Root $ToolRoot -Contract $driftContract
Assert-True -Condition (-not $driftIdentity.valid) -Message 'harness source/checkpoint drift must block'

$lsaSid = $expectedServiceSid
$differentServiceSidFixture = 'S-1-5-80-9999999999-8888888888-7777777777-6666666666-5555'
$lsaBefore = [pscustomobject]@{
    service_sid = $lsaSid
    direct = [pscustomobject]@{ status = 'READ'; direct_rights = @() }
    assignment = [pscustomobject]@{ status = 'READ'; assigned_principals = @() }
}
$lsaAfter = [pscustomobject]@{
    service_sid = $lsaSid
    direct = [pscustomobject]@{ status = 'READ'; direct_rights = @('SeSystemProfilePrivilege') }
    assignment = [pscustomobject]@{ status = 'READ'; assigned_principals = @($lsaSid) }
}
$lsaDecision = Get-Q1LsaMaterializationDecision -Before $lsaBefore -After $lsaAfter -ServiceSid $lsaSid -Right $contract.allowed_lsa_right -MutationAttempted $true
Assert-True -Condition $lsaDecision.valid -Message 'new Q1 Service SID CONTROL right materialization fixture'
Assert-True -Condition $lsaDecision.added_by_run -Message 'Q1 LSA ownership is tracked'
Assert-True -Condition $lsaDecision.cleanup_allowed -Message 'Q1-owned LSA right is removable'
$lsaCleanup = Test-Q1LsaCleanupEvidence -Snapshot $lsaBefore -ServiceSid $lsaSid -Right $contract.allowed_lsa_right
Assert-True -Condition $lsaCleanup.valid -Message 'Q1 LSA cleanup readback fixture'
$lsaPreexisting = [pscustomobject]@{
    direct = [pscustomobject]@{ status = 'READ'; direct_rights = @('SeSystemProfilePrivilege') }
    assignment = [pscustomobject]@{ status = 'READ'; assigned_principals = @($lsaSid) }
}
$lsaPreexistingDecision = Get-Q1LsaMaterializationDecision -Before $lsaPreexisting -After $lsaAfter -ServiceSid $lsaSid -Right $contract.allowed_lsa_right -MutationAttempted $true
Assert-True -Condition (-not $lsaPreexistingDecision.valid) -Message 'pre-existing Q1 LSA right must fail closed'
Assert-True -Condition (-not $lsaPreexistingDecision.cleanup_allowed) -Message 'pre-existing LSA right is not owned by Q1'

$lsaIntentFixture = [pscustomobject]@{
    task_id = $contract.task_id
    service_name = $contract.service_name
    service_sid = $lsaSid
    right = $contract.allowed_lsa_right
    target = $contract.allowed_lsa_target
    mutation_intent = 'ADD_EXACT_RIGHT'
    ownership_before = 'ABSENT'
    right_absent_before = $true
    cleanup_if_ambiguous = 'REQUIRED'
}
$lsaStartedFixture = [pscustomobject]@{
    state = 'MUTATION_ATTEMPTED'
    mutation_attempted = $true
    service_name = $contract.service_name
    service_sid = $lsaSid
    right = $contract.allowed_lsa_right
    target = $contract.allowed_lsa_target
}
$lsaRecoveryPlan = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right -ServiceSidReadback $lsaSid
Assert-True -Condition $lsaRecoveryPlan.valid -Message 'LSA post-add exception has a valid durable recovery plan'
Assert-Equal -Actual $lsaRecoveryPlan.action -Expected 'REMOVE_EXACT_RIGHT' -Message 'LSA post-add exception requires exact removal'
$lsaRecoveryAfterRemoval = [pscustomobject]@{
    direct = [pscustomobject]@{ status = 'READ'; direct_rights = @() }
    assignment = [pscustomobject]@{ status = 'READ'; assigned_principals = @() }
}
$lsaRecoveryCleanup = Test-Q1LsaCleanupEvidence -Snapshot $lsaRecoveryAfterRemoval -ServiceSid $lsaSid -Right $contract.allowed_lsa_right
Assert-True -Condition $lsaRecoveryCleanup.valid -Message 'LSA post-add exception cleanup verifies exact right absence'
$lsaRecoveryWithoutOwnership = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right
Assert-Equal -Actual $lsaRecoveryWithoutOwnership.action -Expected 'REMOVE_EXACT_RIGHT' -Message 'LSA recovery does not depend on ownership JSON return'
$lsaTamperedIntent = $lsaIntentFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$lsaTamperedIntent.service_sid = $differentServiceSidFixture
$lsaTamperedIntentPlan = Get-Q1LsaRecoveryPlan -Intent $lsaTamperedIntent -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right
Assert-True -Condition (-not $lsaTamperedIntentPlan.valid) -Message 'tampered valid Service SID in intent is blocked by controller anchor'
Assert-Equal -Actual $lsaTamperedIntentPlan.action -Expected 'FAILED_CLOSED' -Message 'tampered intent cannot authorize LSA removal'
$lsaTamperedStarted = $lsaStartedFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$lsaTamperedStarted.service_sid = $differentServiceSidFixture
$lsaTamperedStartedPlan = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaTamperedStarted -Before $lsaBefore -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right
Assert-True -Condition (-not $lsaTamperedStartedPlan.valid) -Message 'tampered mutation-started SID is blocked'
$lsaTamperedReadbackPlan = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right -ServiceSidReadback $differentServiceSidFixture
Assert-True -Condition (-not $lsaTamperedReadbackPlan.valid) -Message 'mismatched independent service showsid readback is blocked'
$lsaInvalidAnchorPlan = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid 'S-1-5-19' -ExpectedRight $contract.allowed_lsa_right
Assert-True -Condition (-not $lsaInvalidAnchorPlan.valid) -Message 'non-Service-SID controller anchor is blocked'
$lsaUnavailable = [pscustomobject]@{
    direct = [pscustomobject]@{ status = 'UNAVAILABLE'; direct_rights = @() }
    assignment = [pscustomobject]@{ status = 'UNAVAILABLE'; assigned_principals = @() }
}
$lsaUnavailablePlan = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaUnavailable -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right
Assert-True -Condition (-not $lsaUnavailablePlan.valid) -Message 'unavailable LSA cleanup readback fails closed'
Assert-Equal -Actual $lsaUnavailablePlan.residual_state -Expected 'UNKNOWN' -Message 'unavailable LSA readback remains unknown'
$lsaPreexistingRecovery = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaPreexisting -Current $lsaAfter -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right
Assert-True -Condition (-not $lsaPreexistingRecovery.valid) -Message 'pre-existing LSA right blocks before ownership'
$lsaUnexpectedCurrent = [pscustomobject]@{
    direct = [pscustomobject]@{ status = 'READ'; direct_rights = @('SeSystemProfilePrivilege', 'SeDebugPrivilege') }
    assignment = [pscustomobject]@{ status = 'READ'; assigned_principals = @($lsaSid) }
}
$lsaUnexpectedPlan = Get-Q1LsaRecoveryPlan -Intent $lsaIntentFixture -MutationStarted $lsaStartedFixture -Before $lsaBefore -Current $lsaUnexpectedCurrent -ExpectedServiceName $contract.service_name -ExpectedServiceSid $lsaSid -ExpectedRight $contract.allowed_lsa_right
Assert-True -Condition (-not $lsaUnexpectedPlan.valid) -Message 'unexpected unrelated LSA right blocks recovery'
Assert-Equal -Actual $lsaUnexpectedPlan.action -Expected 'FAILED_CLOSED' -Message 'unexpected unrelated right cannot trigger broad removal'

$sealWithLsaPass = Get-Q1EvidenceSealDecision -WritersQuiesced $true -LsaCleanupVerified $true
Assert-True -Condition $sealWithLsaPass.seal_allowed -Message 'stopped service with successful LSA cleanup may seal evidence'
$sealWithLsaFailure = Get-Q1EvidenceSealDecision -WritersQuiesced $true -LsaCleanupVerified $false
Assert-True -Condition $sealWithLsaFailure.seal_allowed -Message 'LSA cleanup failure does not block evidence sealing'
Assert-True -Condition $sealWithLsaFailure.seal_attempted -Message 'LSA cleanup failure still attempts evidence sealing'
$sealWithWriterActive = Get-Q1EvidenceSealDecision -WritersQuiesced $false -LsaCleanupVerified $false
Assert-True -Condition (-not $sealWithWriterActive.seal_allowed) -Message 'active writer blocks evidence sealing'
Assert-Equal -Actual $sealWithWriterActive.state -Expected 'BLOCKED_WRITER_NOT_QUIESCED' -Message 'active writer sealing state is explicit'
$deleteAfterPass = Get-Q1RegistrationDeletionDecision -WritersQuiesced $true -LsaCleanupVerified $true -EvidenceSealValid $true
Assert-True -Condition $deleteAfterPass.delete_allowed -Message 'service deletion follows verified cleanup'
$keepAfterFailure = Get-Q1RegistrationDeletionDecision -WritersQuiesced $true -LsaCleanupVerified $false -EvidenceSealValid $true
Assert-True -Condition (-not $keepAfterFailure.delete_allowed) -Message 'LSA failure keeps registration for recovery'
Assert-Equal -Actual $keepAfterFailure.registration_state -Expected 'KEPT_FOR_RECOVERY' -Message 'LSA failure registration policy is explicit'

$q1ServiceSidFixture = 'S-1-5-80-1111111111-2222222222-3333333333-4444444444-5555'
$stagingAclModel = Get-Q1OutputAclModel -Phase STAGING
$stagingAclGate = Test-Q1OutputAclModel -Model $stagingAclModel -ExpectedPhase STAGING -ExpectedServiceSid $q1ServiceSidFixture
Assert-True -Condition $stagingAclGate.valid -Message 'staging ACL excludes account-wide LocalService write access'
Assert-True -Condition (-not ($stagingAclModel.rules | Where-Object { $_.identity -ieq 'S-1-5-19' })) -Message 'staging ACL has no LocalService account rule'
$authorizedAclModel = Get-Q1OutputAclModel -Phase AUTHORIZED -ServiceSid $q1ServiceSidFixture
$authorizedAclGate = Test-Q1OutputAclModel -Model $authorizedAclModel -ExpectedPhase AUTHORIZED -ExpectedServiceSid $q1ServiceSidFixture
Assert-True -Condition $authorizedAclGate.valid -Message 'authorized ACL grants exact Q1 Service SID write access'
Assert-True -Condition (-not ($authorizedAclModel.rules | Where-Object { $_.identity -ieq $differentServiceSidFixture })) -Message 'different Service SID is not granted output access'
$sealedAclModel = Get-Q1OutputAclModel -Phase SEALED -ServiceSid $q1ServiceSidFixture
$sealedAclGate = Test-Q1OutputAclModel -Model $sealedAclModel -ExpectedPhase SEALED -ExpectedServiceSid $q1ServiceSidFixture
Assert-True -Condition $sealedAclGate.valid -Message 'sealed ACL removes exact Q1 Service SID write access'

$evidenceFixtureRoot = New-TestRoot
try {
    $evidenceRaw = Join-Path $evidenceFixtureRoot 'raw'
    $evidenceTimechart = Join-Path $evidenceRaw 'timechart-output'
    New-Item -ItemType Directory -Path $evidenceTimechart -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $evidenceFixtureRoot 'manifest.json'), '{"schema":"fixture"}')
    [IO.File]::WriteAllText((Join-Path $evidenceRaw 'token-evidence.json'), '{"session_id":0}')
    [IO.File]::WriteAllText((Join-Path $evidenceTimechart 'timechart.csv'), (Get-OfflineCsvFixtureText))
    $evidenceEntries = @(Get-Q1EvidenceManifestEntries -Root $evidenceFixtureRoot)
    Assert-True -Condition ($evidenceEntries.Count -eq 3) -Message 'evidence manifest inventories root and raw files'
    Assert-True -Condition (@($evidenceEntries | Where-Object { $_.relative_path -eq 'raw/timechart-output/timechart.csv' -and $_.size -gt 0 -and $_.sha256.Length -eq 64 }).Count -eq 1) -Message 'evidence manifest contains path size and SHA256'
    $evidenceManifestFixture = [pscustomobject]@{ entries = $evidenceEntries }
    $evidenceGate = Test-Q1EvidenceManifest -Root $evidenceFixtureRoot -Manifest $evidenceManifestFixture
    Assert-True -Condition $evidenceGate.valid -Message 'evidence manifest hash validation passes'
    [IO.File]::AppendAllText((Join-Path $evidenceTimechart 'timechart.csv'), "`n tamper")
    $tamperGate = Test-Q1EvidenceManifest -Root $evidenceFixtureRoot -Manifest $evidenceManifestFixture
    Assert-True -Condition (-not $tamperGate.valid) -Message 'tampered evidence fails manifest hash validation'
    Assert-True -Condition (Test-Path -LiteralPath $evidenceFixtureRoot -PathType Container) -Message 'failed scientific evidence remains preserved'
}
finally {
    if (Test-Path -LiteralPath $evidenceFixtureRoot) {
        Remove-Item -LiteralPath $evidenceFixtureRoot -Recurse -Force
    }
}

$gateFixtureRoot = New-TestRoot
try {
    $gateFixtureContract = [ordered]@{
        output_base = $gateFixtureRoot
        gate_file_name = 'Q1-LIVE-GATE.json'
        task_id = $contract.task_id
        max_runs = 1
        retries = 0
    }
    $availableGateState = Get-Q1GateState -Contract $gateFixtureContract
    Assert-Equal -Actual $availableGateState.state -Expected 'AVAILABLE' -Message 'read-only gate inspection reports available'
    Assert-True -Condition $availableGateState.valid -Message 'available gate state is valid'
    Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $gateFixtureRoot 'Q1-LIVE-GATE.json'))) -Message 'read-only gate inspection does not create the gate'
    $gatePath = Join-Path $gateFixtureRoot 'Q1-LIVE-GATE.json'
    $gateRecord = New-OneShotGateRecord -TaskId $contract.task_id -RunId 'offline-gate-fixture' -MaxRuns 1 -Retries 0 -RealExecutionAllowed $false
    $firstGate = Acquire-OneShotGateFile -GatePath $gatePath -GateRecord $gateRecord
    Assert-Equal -Actual $firstGate.state -Expected 'CONSUMED' -Message 'first isolated gate is consumed'
    $consumedGateState = Get-Q1GateState -Contract $gateFixtureContract
    Assert-Equal -Actual $consumedGateState.state -Expected 'ALREADY_CONSUMED' -Message 'read-only gate inspection reports consumed'
    Assert-True -Condition $consumedGateState.valid -Message 'consumed gate state is valid'
    $secondGateBlocked = $false
    try {
        Acquire-OneShotGateFile -GatePath $gatePath -GateRecord $gateRecord | Out-Null
    }
    catch {
        $secondGateBlocked = $true
    }
    Assert-True -Condition $secondGateBlocked -Message 'consumed gate rejects second live attempt'
    Assert-True -Condition (Test-Path -LiteralPath $gatePath -PathType Leaf) -Message 'consumed gate remains durable'
    [IO.File]::WriteAllText($gatePath, '{}')
    $invalidGateState = Get-Q1GateState -Contract $gateFixtureContract
    Assert-Equal -Actual $invalidGateState.state -Expected 'INVALID_OR_UNREADABLE' -Message 'malformed gate is reported as invalid'
    Assert-True -Condition (-not $invalidGateState.valid) -Message 'malformed gate fails closed'
}
finally {
    if (Test-Path -LiteralPath $gateFixtureRoot) {
        Remove-Item -LiteralPath $gateFixtureRoot -Recurse -Force
    }
}

$zeroAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
    state = 'NOT_ATTEMPTED'
    process_started = $false
    invocation_attempted = 0
    power_sampling_runs = 0
})
Assert-Equal -Actual $zeroAccounting.invocation_certainty -Expected 'CONFIRMED_ZERO' -Message 'no launch intent is confirmed zero'
Assert-Equal -Actual $zeroAccounting.amd_cli_real_invocations -Expected 0 -Message 'no launch intent has zero AMD invocations'
$startedAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
    state = 'PROCESS_FAILED_AFTER_START'
    process_started = $true
    invocation_attempted = 1
    power_sampling_runs = 1
})
Assert-Equal -Actual $startedAccounting.amd_cli_real_invocations -Expected 1 -Message 'post-start harness error remains one AMD invocation'
Assert-Equal -Actual $startedAccounting.power_sampling_runs -Expected 1 -Message 'post-start harness error remains one sampling run'
$launchEvidenceRoot = New-TestRoot
try {
    $launchEvidenceRaw = Join-Path $launchEvidenceRoot 'raw'
    New-Item -ItemType Directory -Path $launchEvidenceRaw | Out-Null
    $launchOutputDirectory = Join-Path $launchEvidenceRoot 'raw\timechart-output'
    $launchArguments = @(Get-FixedAmdCliArguments -OutputDirectory $launchOutputDirectory)
    $launchIntentFixture = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/cli-launch-intent/v1'
        state = 'LAUNCH_INTENT_DURABLE'
        launch_attempt_permitted = $true
        gate_consumed = $true
        start_result = 'UNKNOWN'
        executable = $contract.amd_cli_path
        arguments = $launchArguments
        working_directory = (Split-Path -Parent $contract.amd_cli_path)
        output_directory = $launchOutputDirectory
    }
    $launchFailedFixture = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/cli-launch-start-failed/v1'
        state = 'PROCESS_START_FAILED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
        launch_attempt_permitted = $true
        gate_consumed = $true
        start_result = 'FAILED'
        executable = $contract.amd_cli_path
        arguments = $launchArguments
        working_directory = (Split-Path -Parent $contract.amd_cli_path)
        output_directory = $launchOutputDirectory
        error = 'offline fixture'
    }
    [IO.File]::WriteAllText((Join-Path $launchEvidenceRaw 'cli-launch-intent.json'), ($launchIntentFixture | ConvertTo-Json -Depth 20))
    $ambiguousAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'NOT_ATTEMPTED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $ambiguousAccounting.invocation_certainty -Expected 'AMBIGUOUS' -Message 'launch intent without successor is ambiguous'
    Assert-Equal -Actual $ambiguousAccounting.amd_cli_real_invocations -Expected 'UNKNOWN_0_OR_1' -Message 'ambiguous launch is not numeric zero'
    Assert-True -Condition $ambiguousAccounting.gate_consumed -Message 'ambiguous launch keeps the one-shot gate consumed'
    Assert-True -Condition $ambiguousAccounting.second_run_forbidden -Message 'ambiguous launch forbids a second run'
    [IO.File]::WriteAllText((Join-Path $launchEvidenceRaw 'cli-launch-start-failed.json'), ($launchFailedFixture | ConvertTo-Json -Depth 20))
    $failedAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'LAUNCH_FAILED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $failedAccounting.invocation_certainty -Expected 'CONFIRMED_ZERO' -Message 'durable start-failed successor is confirmed zero'
    Assert-True -Condition $failedAccounting.launch_failed_valid -Message 'valid start-failed successor is schema and command validated'
    $corruptStartFailed = Join-Path $launchEvidenceRaw 'cli-launch-start-failed.json'
    [IO.File]::WriteAllText($corruptStartFailed, '{not-json')
    $corruptAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'LAUNCH_FAILED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $corruptAccounting.invocation_certainty -Expected 'AMBIGUOUS' -Message 'corrupt start-failed successor is ambiguous'
    [IO.File]::WriteAllText($corruptStartFailed, ($launchFailedFixture | ConvertTo-Json -Depth 20))
    $wrongExecutable = $launchFailedFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $wrongExecutable.executable = 'D:\wrong\AMDuProfCLI.exe'
    [IO.File]::WriteAllText($corruptStartFailed, ($wrongExecutable | ConvertTo-Json -Depth 20))
    $wrongExecutableAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'LAUNCH_FAILED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $wrongExecutableAccounting.invocation_certainty -Expected 'AMBIGUOUS' -Message 'mismatched start-failed executable is ambiguous'
    $wrongArguments = $launchFailedFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $wrongArguments.arguments = @('timechart', '--event', 'frequency')
    [IO.File]::WriteAllText($corruptStartFailed, ($wrongArguments | ConvertTo-Json -Depth 20))
    $wrongArgumentsAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'LAUNCH_FAILED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $wrongArgumentsAccounting.invocation_certainty -Expected 'AMBIGUOUS' -Message 'mismatched start-failed arguments are ambiguous'
    [IO.File]::WriteAllText($corruptStartFailed, ($launchFailedFixture | ConvertTo-Json -Depth 20))
    [IO.File]::WriteAllText((Join-Path $launchEvidenceRaw 'cli-launch-started.json'), '{"process_started":true}')
    $contradictoryAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'NOT_ATTEMPTED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $contradictoryAccounting.invocation_certainty -Expected 'CONFIRMED_ONE' -Message 'started successor dominates contradictory failure successor'
    Assert-True -Condition $contradictoryAccounting.evidence_conflict -Message 'contradictory launch successors are flagged'
    Remove-Item -LiteralPath (Join-Path $launchEvidenceRaw 'cli-launch-start-failed.json') -Force
    Remove-Item -LiteralPath (Join-Path $launchEvidenceRaw 'cli-launch-started.json') -Force
    [IO.File]::WriteAllText((Join-Path $launchEvidenceRaw 'cli-launch-started.json'), '{"process_started":true}')
    $durableAccounting = Get-InvocationAccounting -ProcessResult ([pscustomobject]@{
        state = 'NOT_ATTEMPTED'
        process_started = $false
        invocation_attempted = 0
        power_sampling_runs = 0
    }) -RunRoot $launchEvidenceRoot
    Assert-Equal -Actual $durableAccounting.amd_cli_real_invocations -Expected 1 -Message 'durable launch evidence cannot be downgraded'
    Assert-Equal -Actual $durableAccounting.invocation_certainty -Expected 'CONFIRMED_ONE' -Message 'durable launch evidence is confirmed one'
}
finally {
    if (Test-Path -LiteralPath $launchEvidenceRoot) {
        Remove-Item -LiteralPath $launchEvidenceRoot -Recurse -Force
    }
}

$fixtureRoot = New-TestRoot
try {
    $validRoot = Join-Path $fixtureRoot 'q1-run'
    $rootGate = Test-ControlledRunRoot -RunRoot $validRoot -OutputBase $fixtureRoot
    Assert-True -Condition $rootGate.valid -Message 'new isolated run root'
    New-Item -ItemType Directory -Path $validRoot | Out-Null
    $existingGate = Test-ControlledRunRoot -RunRoot $validRoot -OutputBase $fixtureRoot
    Assert-True -Condition (-not $existingGate.valid) -Message 'pre-existing run root must block'
    $traversal = Test-ControlledRunRoot -RunRoot (Join-Path $fixtureRoot '..\outside') -OutputBase $fixtureRoot
    Assert-True -Condition (-not $traversal.valid) -Message 'run root traversal must block'
}
finally {
    if (Test-Path -LiteralPath $fixtureRoot) {
        Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
    }
}

$validCsvRoot = New-TestRoot
try {
    $validCsv = Join-Path $validCsvRoot 'timechart.csv'
    [IO.File]::WriteAllText($validCsv, (Get-OfflineCsvFixtureText))
    $parsed = Parse-PackagePowerCsv -Path $validCsv
    $powerEvidence = Test-PackagePowerEvidence -Parsed $parsed
    Assert-Equal -Actual $powerEvidence.status -Expected 'PASS' -Message 'finite non-constant package power'
    Assert-Equal -Actual $powerEvidence.sample_count -Expected 3 -Message 'CSV sample count'
    $constantCsv = Join-Path $validCsvRoot 'constant.csv'
    [IO.File]::WriteAllText($constantCsv, @'
PROFILE RECORDS
Timestamp,Record ID,Socket0-Package-Power
2026-01-01T00:00:00Z,1,40
2026-01-01T00:00:01Z,2,40
'@)
    $constantEvidence = Test-PackagePowerEvidence -Parsed (Parse-PackagePowerCsv -Path $constantCsv)
    Assert-Equal -Actual $constantEvidence.status -Expected 'FAIL' -Message 'constant package power rejected'
    Assert-Equal -Actual $constantEvidence.non_constant -Expected $false -Message 'constant flag'
}
finally {
    if (Test-Path -LiteralPath $validCsvRoot) {
        Remove-Item -LiteralPath $validCsvRoot -Recurse -Force
    }
}

Test-PowerShellSyntax -Path $RunnerPath
Test-PowerShellSyntax -Path $ServiceHostPath
$runnerText = Get-Content -LiteralPath $RunnerPath -Raw
$serviceText = Get-Content -LiteralPath $ServiceHostPath -Raw
$contractText = Get-Content -LiteralPath (Join-Path $ToolRoot 'contract.ps1') -Raw
Assert-Contains -Text $runnerText -Needle 'sc.exe' -Message 'live service lifecycle exists'
Assert-Contains -Text $runnerText -Needle 'Get-GitBaselineEvidence' -Message 'baseline pin and clean-tree preflight exists'
Assert-Contains -Text $runnerText -Needle 'Get-RuntimeFailureCategory' -Message 'runtime failure classification exists'
Assert-Contains -Text $runnerText -Needle 'showsid' -Message 'exact Service SID capture exists'
Assert-Contains -Text $runnerText -Needle 'Get-ServiceConfigurationEvidence' -Message 'service configuration validation exists'
Assert-Contains -Text $runnerText -Needle 'Acquire-OneShotGate' -Message 'one-shot gate exists'
Assert-Contains -Text $runnerText -Needle 'New-QualificationServiceCreateArguments' -Message 'qualified sc.exe argv helper is reused'
Assert-Contains -Text $runnerText -Needle 'Initialize-Q1LsaMaterialization' -Message 'CONTROL baseline LSA materialization exists'
Assert-Contains -Text $runnerText -Needle 'Recover-Q1LsaMutationIfNecessary' -Message 'durable LSA recovery path exists'
Assert-Contains -Text $runnerText -Needle '-ExpectedServiceSid $expectedServiceSid' -Message 'LSA recovery uses controller-held Service SID anchor'
Assert-Contains -Text $runnerText -Needle 'raw\service-definition.json' -Message 'independent service-definition SID anchor is durable before LSA mutation'
Assert-Contains -Text $runnerText -Needle 'Test-Q1WriterQuiescence' -Message 'writer quiescence gate exists'
Assert-Contains -Text $runnerText -Needle 'Get-Q1EvidenceSealDecision' -Message 'evidence sealing is independent of LSA cleanup result'
Assert-Contains -Text $runnerText -Needle 'Grant-Q1ServiceSidOutputAccess' -Message 'exact Service SID output authorization exists'
Assert-Contains -Text $runnerText -Needle 'Seal-Q1Evidence' -Message 'raw evidence sealing path exists'
Assert-Contains -Text $runnerText -Needle 'KeepRegistration' -Message 'service deletion is deferred until recovery/sealing'
Assert-Contains -Text $runnerText -Needle 'Test-HarnessSourceIdentity' -Message 'reviewed harness source identity is enforced'
Assert-Contains -Text $runnerText -Needle "ValidateSet('DryRun', 'Preflight', 'Live')" -Message 'read-only host preflight mode is exposed'
Assert-Contains -Text $runnerText -Needle 'Get-Q1GateState' -Message 'read-only Q1 gate inspection exists'
Assert-Contains -Text $runnerText -Needle 'Get-Q1DriverContractValidation' -Message 'driver contract validation is shared with live preflight'
Assert-True -Condition (-not ($runnerText -match '\$Contract\.driver_versions\.PSObject\.Properties')) -Message 'live preflight does not enumerate driver adapter properties'
Assert-Contains -Text $serviceText -Needle 'ServiceBase' -Message 'dedicated ServiceBase host exists'
Assert-Contains -Text $serviceText -Needle 'Get-EffectiveTokenEvidence' -Message 'effective token capture exists'
Assert-Contains -Text $serviceText -Needle 'ExpectedServiceSid' -Message 'exact Service SID token validation exists'
Assert-Contains -Text $serviceText -Needle 'Invoke-BoundedAmdCli' -Message 'bounded child path exists'
Assert-Contains -Text $serviceText -Needle 'taskkill.exe' -Message 'owned process-tree cleanup exists'
Assert-Contains -Text $serviceText -Needle 'cli-launch-started.json' -Message 'irreversible launch evidence exists'
Assert-Contains -Text $serviceText -Needle 'cli-launch-start-failed.json' -Message 'explicit launch-failure successor evidence exists'
Assert-Contains -Text $contractText -Needle 'AMBIGUOUS_AFTER_LAUNCH_INTENT' -Message 'ambiguous launch state is represented'
Assert-Contains -Text $contractText -Needle 'Test-Q1LaunchStartFailedEvidence' -Message 'launch-failure successor schema validation exists'
Assert-Contains -Text $serviceText -Needle 'PROCESS_FAILED_AFTER_START' -Message 'post-start harness failure is classified'
Assert-True -Condition (-not ($runnerText -match 'I2G_REAL_RUN_AUTHORIZATION')) -Message 'I2G authorization marker is not reused'

$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
if (-not (Test-Path -LiteralPath $powershell -PathType Leaf)) {
    $powershell = (Get-Command pwsh -ErrorAction Stop).Source
}
$dryOutput = & $powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $RunnerPath -Mode DryRun 2>&1 | Out-String
$dryExitCode = $LASTEXITCODE
Assert-Equal -Actual $dryExitCode -Expected 0 -Message 'runner dry-run exit code'
$dry = $dryOutput | ConvertFrom-Json
Assert-Equal -Actual $dry.result -Expected 'OFFLINE_VALIDATION_PASS' -Message 'runner dry-run result'
Assert-Equal -Actual $dry.plan.command.arguments[0] -Expected 'timechart' -Message 'dry-run command verb'
Assert-Equal -Actual $dry.plan.command.arguments[2] -Expected 'power' -Message 'dry-run package power event'
Assert-Equal -Actual $dry.plan.gate.max_runs -Expected 1 -Message 'dry-run gate max runs'
Assert-Equal -Actual $dry.plan.gate.retries -Expected 0 -Message 'dry-run gate retries'
Assert-Equal -Actual $dry.amd_cli_real_invocations -Expected 0 -Message 'dry-run AMD CLI count'
Assert-Equal -Actual $dry.power_sampling_runs -Expected 0 -Message 'dry-run sampling count'
Assert-Equal -Actual $dry.current_task_lsa_mutations -Expected 0 -Message 'offline current-task LSA mutation count'
Assert-Equal -Actual $dry.harness_live_contract_allows_control_baseline_lsa_mutation -Expected 'YES' -Message 'live LSA capability is distinct from offline count'
Assert-Equal -Actual $dry.allowed_lsa_right -Expected 'SeSystemProfilePrivilege' -Message 'dry-run allowed LSA right'
Assert-Equal -Actual $dry.allowed_lsa_target -Expected 'EXACT_Q1_SERVICE_SID_ONLY' -Message 'dry-run allowed LSA target'
Assert-True -Condition (-not (Test-Path -LiteralPath $contract.output_base)) -Message 'dry-run did not create ProgramData output base'
$preflightGateBefore = Get-Q1GateState -Contract $contract
$preflightOutput = & $powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $RunnerPath -Mode Preflight 2>&1 | Out-String
$preflightExitCode = $LASTEXITCODE
Assert-True -Condition (@(0, 1) -contains $preflightExitCode) -Message 'read-only preflight returns a controlled status code'
$preflight = $preflightOutput | ConvertFrom-Json
Assert-True -Condition (@('PREFLIGHT_PASS', 'PREFLIGHT_BLOCKED') -contains [string]$preflight.result) -Message 'read-only preflight returns a structured result'
Assert-Equal -Actual $preflight.mode -Expected 'PREFLIGHT' -Message 'read-only preflight mode'
Assert-Equal -Actual $preflight.authorization_required -Expected $false -Message 'preflight does not require live authorization'
Assert-Equal -Actual $preflight.amd_cli_real_invocations -Expected 0 -Message 'preflight AMD CLI count'
Assert-Equal -Actual $preflight.power_sampling_runs -Expected 0 -Message 'preflight sampling count'
Assert-Equal -Actual $preflight.service_mutations -Expected 0 -Message 'preflight service mutation count'
Assert-Equal -Actual $preflight.lsa_mutations -Expected 0 -Message 'preflight LSA mutation count'
Assert-Equal -Actual $preflight.acl_mutations -Expected 0 -Message 'preflight ACL mutation count'
Assert-Equal -Actual $preflight.candidate_run_root_created -Expected $false -Message 'preflight does not create a candidate run root'
Assert-True -Condition (-not $preflight.candidate_run_root_exists) -Message 'preflight candidate run root remains absent'
Assert-True -Condition (-not $preflight.service_lifecycle.created) -Message 'preflight does not create a service'
Assert-Equal -Actual $preflight.q1_gate_state -Expected $preflight.preflight.q1_gate.state -Message 'preflight reports the same read-only gate state'
$preflightGateAfter = Get-Q1GateState -Contract $contract
Assert-Equal -Actual $preflightGateAfter.state -Expected $preflightGateBefore.state -Message 'preflight does not consume or alter the Q1 gate'
Assert-Equal -Actual $preflightGateAfter.exists -Expected $preflightGateBefore.exists -Message 'preflight does not create the Q1 gate'

$previousErrorActionPreference = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
$liveOutput = & $powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $RunnerPath -Mode Live 2>&1 | Out-String
$ErrorActionPreference = $previousErrorActionPreference
$liveExitCode = $LASTEXITCODE
Assert-True -Condition ($liveExitCode -ne 0) -Message 'live mode without dedicated authorization must fail closed'
Assert-Contains -Text $liveOutput -Needle 'AuthorizeLiveRun' -Message 'live authorization failure is explicit'
$residualService = Get-CimInstance Win32_Service -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -eq $contract.service_name }
Assert-True -Condition ($null -eq $residualService) -Message 'unauthorized live path created no service'
$residualProcess = Get-Process -Name 'AMDuProfCLI', 'AMDuProf' -ErrorAction SilentlyContinue
Assert-True -Condition ($null -eq $residualProcess) -Message 'offline tests started no AMD process'

[pscustomobject]@{
    result = 'AMD_LOCAL_SERVICE_ACTIVE_SAMPLING_HARNESS_OFFLINE_TEST_PASS'
    task_id = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-HARNESS-I1'
    dry_run = 'PASS'
    syntax = 'PASS'
    command_contract = 'PASS'
    token_contract = 'PASS'
    binary_driver_contract = 'PASS'
    ordered_dictionary_driver_enumeration = 'PASS'
    host_driver_evidence_behavior = 'PASS'
    read_only_preflight_mode = 'PASS'
    csv_validation = 'PASS'
    lsa_recovery_fault_injection = 'PASS'
    independent_service_sid_anchor = 'PASS'
    seal_on_lsa_cleanup_failure = 'PASS'
    writer_quiescence_seal_gate = 'PASS'
    exact_service_sid_acl = 'PASS'
    evidence_manifest_sealing = 'PASS'
    launch_failure_successor_validation = 'PASS'
    invocation_ambiguity_accounting = 'PASS'
    live_default_fail_closed = 'PASS'
    amd_cli_real_invocations = 0
    amd_api_real_invocations = 0
    power_sampling_runs = 0
    current_task_service_mutations = 0
    current_task_lsa_mutations = 0
    current_task_token_mutations = 0
    current_task_acl_mutations = 0
    current_task_device_mutations = 0
    current_task_driver_mutations = 0
    current_task_platform_security_mutations = 0
    live_service_mutation_supported = 'YES'
    live_output_acl_mutation_supported = 'YES'
    live_control_baseline_lsa_mutation_supported = 'YES'
    allowed_lsa_right = 'SeSystemProfilePrivilege'
    allowed_lsa_target = 'EXACT_Q1_SERVICE_SID_ONLY'
} | ConvertTo-Json -Depth 10
