#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedExperiment,
    [switch]$LibraryOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Reuse the reviewed I2E administrative, SCM, AMD preflight, and exact-right helpers.  The
# I2F globals below deliberately retarget those helpers to a new qualification-only service;
# no I2E phase or I2E evidence root is reused.
$I2eSetupPath = Join-Path $PSScriptRoot 'run-admin-amd-i2e-service-profile-experiment.ps1'
. $I2eSetupPath -LibraryOnly
. (Join-Path $PSScriptRoot 'i2f-service-profile-contract.ps1')

$ServiceName = $I2fServiceName
$ServiceAccount = $I2fServiceAccount
$ServiceSidAccount = $I2fServiceSidAccount
$ScServiceAccount = 'NT AUTHORITY\LocalService'
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = 'F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D'
$QualificationRoot = Join-Path $env:ProgramData $I2fOutputSubdirectory
$ConfigPath = Join-Path $QualificationRoot 'I2F-CONFIG.json'
$ControlPreflightPath = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-profile\3935ac9082954bcfb2b1f94c54cf95d7\AMD-CLI-PREFLIGHT.json'

function Write-I2fJson {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
}

function Read-I2fJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required I2F evidence is absent: $Path"
    }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Assert-I2fArtifact {
    if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
        throw "Missing I2F qualification artifact: $ArtifactPath"
    }
    $hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($hash -cne $ExpectedArtifactSha256.ToUpperInvariant()) {
        throw "I2F artifact hash mismatch; expected=$ExpectedArtifactSha256 actual=$hash"
    }
    if ((Get-I2ePeArchitecture -Path $ArtifactPath) -cne 'x64') {
        throw 'I2F qualification artifact must be x64.'
    }
    $hash
}

function Get-I2fControlAmdIdentity {
    $control = Read-I2fJson -Path $ControlPreflightPath
    if (-not [bool](Get-I2ePropertyValue -Object $control -Name 'preflight_pass')) {
        throw "Immutable I2E CONTROL AMD CLI preflight is not passing: $ControlPreflightPath"
    }
    $control
}

function Compare-I2fCurrentAmdIdentity {
    param([Parameter(Mandatory = $true)]$Control)
    $current = Get-I2eAmdCliPreflight
    $comparison = Compare-I2eAmdCliPreflight -Control $Control -Current $current
    [pscustomobject]@{
        control_preflight_path = $ControlPreflightPath
        control_identity = $Control
        current_identity = $current
        comparison = $comparison
        pass = [bool]$comparison.pass
    }
}

function Get-I2fServiceSid {
    $sid = ([Security.Principal.NTAccount]::new($ServiceSidAccount)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    if ($sid -notmatch '^S-1-5-80-') { throw "Unexpected I2F Service SID: $sid" }
    $sid
}

function Assert-I2fNoOwnedProcesses {
    param([Parameter(Mandatory = $true)][string]$AmdCliPath)
    $brokerCount = @(Get-I2eOwnedBrokerProcesses).Count
    $amdCount = @(Get-I2eOwnedAmdProcesses -ExpectedAmdCliPath $AmdCliPath).Count
    if ($brokerCount -ne 0 -or $amdCount -ne 0) {
        throw "I2F owned-process gate failed; broker=$brokerCount amd_cli=$amdCount"
    }
    [pscustomobject]@{
        owned_broker_process_count = $brokerCount
        amd_cli_process_count = $amdCount
    }
}

function Get-I2fRightState {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)
    $direct = Get-I2eDirectAccountRightsSnapshot -Label 'i2f-service-sid' -Sid $ServiceSid
    $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2fRequiredRight
    if ($direct.status -ne 'READ' -or $assigned.status -ne 'READ') {
        throw 'I2F LSA readback was unavailable; failing closed.'
    }
    [pscustomobject]@{
        direct = $direct
        assigned = $assigned
        direct_present = @($direct.direct_rights | Where-Object { $_ -ieq $I2fRequiredRight }).Count -gt 0
        assignment_present = @($assigned.assigned_principals | Where-Object { $_ -ieq $ServiceSid }).Count -gt 0
    }
}

function Assert-I2fRightAbsent {
    param([Parameter(Mandatory = $true)][string]$ServiceSid)
    $state = Get-I2fRightState -ServiceSid $ServiceSid
    if ($state.direct_present -or $state.assignment_present) {
        throw 'I2F Service SID already has SeSystemProfilePrivilege; refusing pre-existing mutation.'
    }
    $state
}

function Wait-I2fServiceEvidence {
    param([Parameter(Mandatory = $true)][string]$OutputRoot)
    $resultPath = Join-Path $OutputRoot 'I2F-COUNTER-DISCOVERY-RESULT.json'
    $errorPath = Join-Path $OutputRoot 'I2F-SERVICE-HARNESS-ERROR.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    do {
        if ((Test-Path -LiteralPath $resultPath -PathType Leaf) -or
            (Test-Path -LiteralPath $errorPath -PathType Leaf)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    if (Test-Path -LiteralPath $errorPath -PathType Leaf) {
        throw "I2F service harness failed: $(Get-Content -LiteralPath $errorPath -Raw)"
    }
    if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
        throw 'I2F did not produce a bounded counter-discovery result.'
    }
    Read-I2fJson -Path $resultPath
}

function Write-I2fRollbackEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][bool]$RightAdded,
        [Parameter(Mandatory = $true)][bool]$PolicyRollbackVerified,
        [Parameter(Mandatory = $true)][bool]$EffectiveTokenTeardownVerified,
        [Parameter(Mandatory = $true)][bool]$ServiceRegistrationRemoved,
        [Parameter(Mandatory = $true)]$StopEvidence,
        [Parameter(Mandatory = $true)]$ProcessEvidence,
        [Parameter(Mandatory = $true)]$RightState
    )
    $fullRollback = $PolicyRollbackVerified -and $EffectiveTokenTeardownVerified -and
        $ServiceRegistrationRemoved
    Write-I2fJson -Path (Join-Path $OutputRoot 'I2F-ROLLBACK.json') -Value ([ordered]@{
            schema = 'amd-i2f-rollback/v1'
            qualification_only = $true
            right = $I2fRequiredRight
            right_added_by_experiment = $RightAdded
            all_rights = $false
            service_stop_attempted = $true
            service_stop_verified = [bool]($StopEvidence.state -eq 'Stopped' -and $StopEvidence.process_id -eq 0)
            service_state_after_stop = $StopEvidence.state
            service_pid_after_stop = $StopEvidence.process_id
            owned_broker_process_count_after_stop = $ProcessEvidence.owned_broker_process_count
            amd_cli_process_count_after_stop = $ProcessEvidence.amd_cli_process_count
            policy_remove_attempted = $RightAdded
            policy_rollback_verified = $PolicyRollbackVerified
            direct_verification = $RightState.direct
            assignment_verification = $RightState.assigned
            effective_token_teardown_verified = $EffectiveTokenTeardownVerified
            full_rollback_verified = $fullRollback
            rollback_verified = $fullRollback
            service_registration_removed = $ServiceRegistrationRemoved
            rollback_at_utc = [DateTime]::UtcNow.ToString('o')
        })
    $fullRollback
}

if ($LibraryOnly) { return }

$null = Assert-I2eAdministrator
if (-not $ExecuteAuthorizedExperiment) {
    Get-I2fExperimentPlan -ArtifactSha256 $ExpectedArtifactSha256 | ConvertTo-Json -Depth 20
    Write-Host 'I2F_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, token adjustment, or AMD runtime was performed.'
    return
}

$artifactHash = Assert-I2fArtifact
if ((Get-I2eServiceSnapshot).present) { throw "I2F service already exists: $ServiceName" }
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) { throw "Stale I2F config exists: $ConfigPath" }
$controlIdentity = Get-I2fControlAmdIdentity
$identityCheck = Compare-I2fCurrentAmdIdentity -Control $controlIdentity
$scope = [Guid]::NewGuid().ToString('N')
$outputRoot = Join-Path $QualificationRoot $scope
New-Item -ItemType Directory -Force -Path $QualificationRoot, $outputRoot | Out-Null
Write-I2fJson -Path (Join-Path $outputRoot 'I2F-AMD-CLI-PREFLIGHT.json') -Value ([ordered]@{
        schema = 'amd-i2f-amd-cli-preflight-comparison/v1'
        qualification_only = $true
        control_preflight_path = $ControlPreflightPath
        experiment_id = $scope
        control_identity = $identityCheck.control_identity
        current_identity = $identityCheck.current_identity
        comparison = $identityCheck.comparison
        pass = $identityCheck.pass
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
if (-not $identityCheck.pass) {
    throw "I2F AMD CLI identity drift detected: $($identityCheck.comparison.differing_fields -join ', ')"
}

$serviceCreated = $false
$rightAdded = $false
$serviceSid = $null
$amdCliPath = [string]$identityCheck.current_identity.path
$stopEvidence = [pscustomobject]@{ state = 'NOT_ATTEMPTED'; process_id = 0L }
$processEvidence = [pscustomobject]@{ owned_broker_process_count = 0; amd_cli_process_count = 0 }
$policyRollbackVerified = $false
$serviceRegistrationRemoved = $false
$fullRollbackVerified = $false
$rightState = $null

try {
    $binPath = '"{0}" --service-profile-enable-counter-service' -f $ArtifactPath
    $createArgs = New-QualificationServiceCreateArguments -ServiceName $ServiceName -BinPath $binPath `
        -ServiceAccount $ScServiceAccount -DisplayName 'Resource Timeline AMD I2F self-enable qualification'
    Invoke-I2eSc -Arguments $createArgs | Out-Null
    $serviceCreated = $true
    Invoke-I2eSc -Arguments @('sidtype', $ServiceName, 'unrestricted') | Out-Null
    Assert-I2eServiceSidType
    $serviceSid = Get-I2fServiceSid
    Set-I2eDirectoryAcl -Path $QualificationRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $outputRoot -ServiceSid $serviceSid
    $config = Get-I2fServiceConfig -Scope $scope -OutputRoot $outputRoot -ServiceSid $serviceSid `
        -AmdCliPreflight $identityCheck.current_identity
    Write-I2fJson -Path $ConfigPath -Value $config
    Write-I2fJson -Path (Join-Path $outputRoot 'I2F-CONFIG.json') -Value $config

    $rightState = Assert-I2fRightAbsent -ServiceSid $serviceSid
    Write-I2fJson -Path (Join-Path $outputRoot 'I2F-LSA-BEFORE.json') -Value $rightState
    Add-I2eExactServiceProfileRight -ServiceSid $serviceSid
    $rightAdded = $true
    $afterAdd = Get-I2fRightState -ServiceSid $serviceSid
    if (-not $afterAdd.direct_present -or -not $afterAdd.assignment_present) {
        throw 'I2F exact-right dual verification failed after LSA assignment.'
    }
    Write-I2fJson -Path (Join-Path $outputRoot 'I2F-LSA-AFTER-ADD.json') -Value $afterAdd

    Invoke-I2eSc -Arguments @('start', $ServiceName) | Out-Null
    $result = Wait-I2fServiceEvidence -OutputRoot $outputRoot
    $stopEvidence = Stop-I2eService
    $processEvidence = Assert-I2fNoOwnedProcesses -AmdCliPath $amdCliPath
}
finally {
    if ($serviceCreated) {
        try { $stopEvidence = Stop-I2eService } catch { }
        try { $processEvidence = Assert-I2fNoOwnedProcesses -AmdCliPath $amdCliPath } catch { }
    }
    if ($rightAdded -and $null -ne $serviceSid) {
        try {
            Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
            $rightState = Get-I2fRightState -ServiceSid $serviceSid
            $policyRollbackVerified = -not $rightState.direct_present -and -not $rightState.assignment_present
        } catch {
            $policyRollbackVerified = $false
            $rightState = [pscustomobject]@{ error = $_.Exception.Message }
        }
    }
    if ($serviceCreated) {
        try {
            Remove-I2eService
            $serviceRegistrationRemoved = -not (Get-I2eServiceSnapshot).present
        } catch { $serviceRegistrationRemoved = $false }
    }
    if ($null -ne $serviceSid -and (Test-Path -LiteralPath $outputRoot -PathType Container)) {
        $effectiveTeardown = ($stopEvidence.state -eq 'Stopped' -and $stopEvidence.process_id -eq 0 -and
            $processEvidence.owned_broker_process_count -eq 0 -and
            $processEvidence.amd_cli_process_count -eq 0)
        if ($null -ne $rightState) {
            $fullRollbackVerified = Write-I2fRollbackEvidence -OutputRoot $outputRoot -RightAdded $rightAdded `
                -PolicyRollbackVerified $policyRollbackVerified `
                -EffectiveTokenTeardownVerified $effectiveTeardown `
                -ServiceRegistrationRemoved $serviceRegistrationRemoved `
                -StopEvidence $stopEvidence -ProcessEvidence $processEvidence -RightState $rightState
            Write-Host "I2F_FULL_ROLLBACK_VERIFIED=$fullRollbackVerified"
        }
    }
}

if (-not $fullRollbackVerified) {
    throw 'I2F cleanup did not establish full rollback; evidence remains open for human recovery.'
}

Remove-Item -LiteralPath $ConfigPath -Force -ErrorAction SilentlyContinue
Write-Host "I2F_SCOPE=$scope"
Write-Host "I2F_RESULT=$($result.availability)"
Write-Host 'I2F_REAL_EXECUTION_COMPLETED=true'
