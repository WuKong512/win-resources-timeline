#requires -Version 5.1
[CmdletBinding()]
param([switch]$ExecuteAuthorizedExperiment)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'sc-argument-contract.ps1')
. (Join-Path $PSScriptRoot 'cleanup-state-contract.ps1')
. (Join-Path $PSScriptRoot 'i2e-service-profile-contract.ps1')

$ServiceName = $I2eServiceName
$ServiceAccount = $I2eServiceAccount
$ScServiceAccount = 'LocalService'
$ServiceSidAccount = $I2eServiceSidAccount
$ArtifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
$ExpectedArtifactSha256 = '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9'
$QualificationRoot = Join-Path $env:ProgramData $I2eOutputSubdirectory
$ConfigPath = Join-Path $QualificationRoot 'I2E-CONFIG.json'
$PointerPath = Join-Path $QualificationRoot 'I2E-EXPERIMENT-CURRENT.json'

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
    $sid = ([Security.Principal.NTAccount]::new($ServiceSidAccount)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    if ($sid -notmatch '^S-1-5-80-') { throw ('Unexpected Service SID: {0}' -f $sid) }
    $sid
}

function Assert-I2eServiceSidType {
    $output = @(Invoke-I2eSc -Arguments @('qsidtype', $ServiceName))
    if (($output -join [Environment]::NewLine) -notmatch '(?i)\bUNRESTRICTED\b') {
        throw ('Service SID type was not verified as UNRESTRICTED: {0}' -f ($output -join ' '))
    }
}

function Stop-I2eService {
    $initial = Get-I2eServiceSnapshot
    if (-not $initial.present) {
        return [pscustomobject]@{ stop_exit_code = 1062; state = 'ABSENT'; process_id = 0L; disposition = 'SERVICE_ABSENT' }
    }
    $sc = Join-Path $env:SystemRoot 'System32\sc.exe'
    & $sc stop $ServiceName | Out-Null
    $exitCode = [int]$LASTEXITCODE
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $current = Get-I2eServiceSnapshot
        if (-not $current.present -or ($current.state -eq 'Stopped' -and $current.process_id -eq 0)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $current = Get-I2eServiceSnapshot
    $state = if ($current.present) { $current.state } else { 'ABSENT' }
    $pid = if ($current.present) { [int64]$current.process_id } else { 0L }
    $disposition = Resolve-QualificationStopDisposition -StopExitCode $exitCode -ServiceState $state -ServiceProcessId $pid -ServicePresent $current.present
    if ($disposition -eq 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0') {
        throw ('I2E service did not stop safely; sc.exe exit={0}, state={1}, pid={2}' -f $exitCode, $state, $pid)
    }
    [pscustomobject]@{ stop_exit_code = $exitCode; state = $state; process_id = $pid; disposition = $disposition }
}

function Remove-I2eService {
    $current = Get-I2eServiceSnapshot
    if (-not $current.present) { return }
    if ($current.state -ne 'Stopped' -or $current.process_id -ne 0) { throw 'Refusing to delete a running I2E service.' }
    Invoke-I2eSc -Arguments @('delete', $ServiceName) | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if (-not (Get-I2eServiceSnapshot).present) { return }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw ('I2E service registration remains: {0}' -f $ServiceName)
}

function Invoke-I2ePhase {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('CONTROL', 'TREATMENT')][string]$Phase,
        [Parameter(Mandatory = $true)][string]$Scope,
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )
    $config = Get-I2ePhaseConfig -Phase $Phase -Scope $Scope -OutputRoot $OutputRoot -ServiceSid $ServiceSid
    Write-I2eJson -Path $ConfigPath -Value $config
    Write-I2eJson -Path (Join-Path $OutputRoot 'I2E-PHASE-CONFIG.json') -Value $config
    Invoke-I2eSc -Arguments @('start', $ServiceName) | Out-Null
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    do {
        $summaryPath = Join-Path $OutputRoot 'SERVICE-PROFILE-COUNTER-SUMMARY.json'
        $errorPath = Join-Path $OutputRoot 'SERVICE-PROFILE-SERVICE-HARNESS-ERROR.json'
        if ((Test-Path -LiteralPath $summaryPath -PathType Leaf) -or (Test-Path -LiteralPath $errorPath -PathType Leaf)) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $errorPath = Join-Path $OutputRoot 'SERVICE-PROFILE-SERVICE-HARNESS-ERROR.json'
    if (Test-Path -LiteralPath $errorPath -PathType Leaf) {
        throw ('{0} service harness failed: {1}' -f $Phase, (Get-Content -LiteralPath $errorPath -Raw))
    }
    $summaryPath = Join-Path $OutputRoot 'SERVICE-PROFILE-COUNTER-SUMMARY.json'
    if (-not (Test-Path -LiteralPath $summaryPath -PathType Leaf)) {
        throw ('{0} did not produce a bounded counter-discovery result.' -f $Phase)
    }
    $stop = Stop-I2eService
    [pscustomobject]@{
        phase = $Phase
        output_root = $OutputRoot
        summary = Read-I2eJson -Path $summaryPath
        context = Read-I2eJson -Path (Join-Path $OutputRoot 'SERVICE-PROFILE-SERVICE-CONTEXT.json')
        token_gate = Read-I2eJson -Path (Join-Path $OutputRoot 'SERVICE-PROFILE-TOKEN-GATE.json')
        stop = $stop
    }
}

$null = Assert-I2eAdministrator
if (-not $ExecuteAuthorizedExperiment) {
    Get-I2eExperimentPlan -ArtifactSha256 $ExpectedArtifactSha256 | ConvertTo-Json -Depth 20
    Write-Host 'I2E_PLAN_ONLY=true'
    Write-Host 'No service, LSA mutation, or AMD runtime was performed.'
    return
}

if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) { throw ('Missing artifact: {0}' -f $ArtifactPath) }
$artifactHash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToUpperInvariant()
if ($artifactHash -ne $ExpectedArtifactSha256.ToUpperInvariant()) {
    throw ('Artifact hash mismatch; expected={0}, actual={1}' -f $ExpectedArtifactSha256, $artifactHash)
}
if ((Get-I2ePeArchitecture -Path $ArtifactPath) -ne 'x64') { throw 'I2E artifact must be x64.' }
if ((Get-I2eServiceSnapshot).present) { throw ('Service already exists: {0}' -f $ServiceName) }
if (Test-Path -LiteralPath $ConfigPath -PathType Leaf) { throw ('Stale config exists: {0}' -f $ConfigPath) }
if (Test-Path -LiteralPath $PointerPath -PathType Leaf) { throw ('Stale experiment pointer exists: {0}' -f $PointerPath) }
$amdCliPreflight = Get-I2eAmdCliPreflight
if (-not $amdCliPreflight.preflight_pass) { throw 'AMD CLI preflight failed.' }

$experimentId = [Guid]::NewGuid().ToString('N')
$controlScope = [Guid]::NewGuid().ToString('N')
$treatmentScope = [Guid]::NewGuid().ToString('N')
$experimentRoot = Join-Path $QualificationRoot $experimentId
$controlRoot = Join-Path $QualificationRoot $controlScope
$treatmentRoot = Join-Path $QualificationRoot $treatmentScope
$serviceCreated = $false
$rightAdded = $false
$rollbackVerified = $false
$serviceSid = $null
$primaryError = $null
$cleanupError = $null
$pointer = [ordered]@{
    schema = 'amd-service-profile-experiment-current/v1'
    qualification_only = $true
    experiment_id = $experimentId
    service_name = $ServiceName
    service_account = $ServiceAccount
    service_account_sid = $I2eServiceAccountSid
    service_sid = $null
    service_sid_account = $ServiceSidAccount
    right = $I2eRequiredRight
    artifact_path = $ArtifactPath
    artifact_sha256 = $artifactHash
    control_scope = $controlScope
    control_output_root = $controlRoot
    treatment_scope = $treatmentScope
    treatment_output_root = $treatmentRoot
    right_added_by_experiment = $false
    rollback_verified = $false
}

try {
    # AUTHORIZED_ORDER: SERVICE_CREATE < SIDTYPE_UNRESTRICTED < QSIDTYPE_VERIFY < SERVICE_SID_RESOLUTION < ACL_CONFIG < CONTROL < RIGHT_MUTATION < TREATMENT < ROLLBACK < SERVICE_DELETE
    New-Item -ItemType Directory -Force -Path $QualificationRoot, $experimentRoot, $controlRoot, $treatmentRoot | Out-Null
    Write-I2eJson -Path $PointerPath -Value $pointer
    $binPath = '"{0}" --service-profile-counter-service' -f $ArtifactPath
    $createArgs = New-QualificationServiceCreateArguments -ServiceName $ServiceName -BinPath $binPath -ServiceAccount $ScServiceAccount -DisplayName 'Resource Timeline AMD service-profile qualification'
    Invoke-I2eSc -Arguments $createArgs | Out-Null
    $serviceCreated = $true
    Invoke-I2eSc -Arguments @('sidtype', $ServiceName, 'unrestricted') | Out-Null
    Assert-I2eServiceSidType
    $serviceSid = Resolve-I2eServiceSid
    $pointer.service_sid = $serviceSid
    Write-I2eJson -Path $PointerPath -Value $pointer
    Set-I2eDirectoryAcl -Path $QualificationRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $experimentRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $controlRoot -ServiceSid $serviceSid
    Set-I2eDirectoryAcl -Path $treatmentRoot -ServiceSid $serviceSid
    Write-I2eJson -Path (Join-Path $experimentRoot 'AMD-CLI-PREFLIGHT.json') -Value $amdCliPreflight

    $baseline = Get-I2eDirectAccountRightsSnapshot -Label 'dedicated-service-sid-before' -Sid $serviceSid
    $assigned = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($baseline.status -ne 'READ' -or $assigned.status -ne 'READ') { throw 'I2E baseline rights could not be read.' }
    $rightPresentBefore = @($baseline.direct_rights) -contains $I2eRequiredRight
    $rightAssignedBefore = @($assigned.assigned_principals) -contains $serviceSid
    if ($rightPresentBefore -or $rightAssignedBefore) {
        throw 'Dedicated Service SID already has SeSystemProfilePrivilege; refusing unknown pre-existing assignment.'
    }
    $plan = Get-I2eExperimentPlan -ArtifactSha256 $artifactHash
    $plan.experiment_id = $experimentId
    $plan.service_sid = $serviceSid
    $plan.control_scope = $controlScope
    $plan.treatment_scope = $treatmentScope
    $plan.baseline_direct_rights = $baseline.direct_rights
    $plan.baseline_account_object_state = $baseline.account_object_state
    $plan.baseline_direct_rights_status = $baseline.status
    $plan.baseline_right_assignment = $assigned
    $plan.se_system_profile_privilege_present_before = $rightPresentBefore
    $plan.right_was_present_before = $rightPresentBefore -or $rightAssignedBefore
    Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-PLAN.json') -Value $plan

    $control = Invoke-I2ePhase -Phase CONTROL -Scope $controlScope -OutputRoot $controlRoot -ServiceSid $serviceSid
    Write-I2eJson -Path (Join-Path $experimentRoot 'CONTROL-RESULT.json') -Value $control.summary
    if ([string]$control.summary.availability -cne 'POWER_UNAVAILABLE') {
        throw ('Control result was {0}; treatment is not authorized.' -f $control.summary.availability)
    }

    Add-I2eExactServiceProfileRight -ServiceSid $serviceSid
    $rightAdded = $true
    $pointer.right_added_by_experiment = $true
    Write-I2eJson -Path $PointerPath -Value $pointer
    $afterAdd = Get-I2eDirectAccountRightsSnapshot -Label 'dedicated-service-sid-after-add' -Sid $serviceSid
    $assignmentAfterAdd = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
    if ($afterAdd.status -ne 'READ' -or $assignmentAfterAdd.status -ne 'READ' -or
        @($afterAdd.direct_rights) -notcontains $I2eRequiredRight -or
        @($assignmentAfterAdd.assigned_principals) -notcontains $serviceSid) {
        throw 'Exact service-SID right was not verified after mutation.'
    }
    Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-APPLIED.json') -Value ([ordered]@{
        schema = 'amd-service-profile-security-mutation-applied/v1'
        qualification_only = $true
        experiment_id = $experimentId
        service_name = $ServiceName
        service_sid = $serviceSid
        right = $I2eRequiredRight
        right_was_present_before = $rightPresentBefore -or $rightAssignedBefore
        right_added_by_experiment = $true
        applied_at_utc = [DateTime]::UtcNow.ToString('o')
        direct_verification = $afterAdd
        assignment_verification = $assignmentAfterAdd
    })

    $treatment = Invoke-I2ePhase -Phase TREATMENT -Scope $treatmentScope -OutputRoot $treatmentRoot -ServiceSid $serviceSid
    Write-I2eJson -Path (Join-Path $experimentRoot 'TREATMENT-RESULT.json') -Value $treatment.summary
    $tokenDelta = Compare-I2eTokenDelta -ControlContext $control.context -TreatmentContext $treatment.context
    Write-I2eJson -Path (Join-Path $experimentRoot 'TOKEN-DELTA.json') -Value $tokenDelta
    if (-not $tokenDelta.pass) { throw 'Treatment token delta was not exactly SeSystemProfilePrivilege.' }
    $pointer.control_result = [string]$control.summary.availability
    $pointer.treatment_result = [string]$treatment.summary.availability
    $pointer.token_delta_verified = $true
    Write-I2eJson -Path $PointerPath -Value $pointer
    Write-Host ('I2E paired experiment completed; evidence root: {0}' -f $experimentRoot)
}
catch {
    $primaryError = $_.Exception
}
finally {
    try {
        if ($rightAdded -and $null -ne $serviceSid) {
            Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
            $afterRollback = Get-I2eDirectAccountRightsSnapshot -Label 'dedicated-service-sid-after-rollback' -Sid $serviceSid
            $assignmentAfterRollback = Get-I2eUserRightAssignmentSnapshot -Right $I2eRequiredRight
            if ($afterRollback.status -ne 'READ' -or $assignmentAfterRollback.status -ne 'READ' -or
                @($afterRollback.direct_rights) -contains $I2eRequiredRight -or
                @($assignmentAfterRollback.assigned_principals) -contains $serviceSid) {
                throw 'Exact service-SID right rollback was not verified.'
            }
            $rollbackVerified = $true
            Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value ([ordered]@{
                schema = 'amd-service-profile-security-mutation-rollback/v1'
                qualification_only = $true
                experiment_id = $experimentId
                service_name = $ServiceName
                service_sid = $serviceSid
                right = $I2eRequiredRight
                right_added_by_experiment = $true
                rollback_at_utc = [DateTime]::UtcNow.ToString('o')
                rollback_verified = $true
                direct_verification = $afterRollback
                assignment_verification = $assignmentAfterRollback
            })
        }
        else {
            $rollbackVerified = $true
            Write-I2eJson -Path (Join-Path $experimentRoot 'SECURITY-MUTATION-ROLLBACK.json') -Value ([ordered]@{
                schema = 'amd-service-profile-security-mutation-rollback/v1'
                qualification_only = $true
                experiment_id = $experimentId
                service_name = $ServiceName
                service_sid = $serviceSid
                right = $I2eRequiredRight
                right_added_by_experiment = $false
                rollback_at_utc = [DateTime]::UtcNow.ToString('o')
                rollback_verified = $true
                verification = 'No security mutation was applied.'
            })
        }
        $pointer.rollback_verified = $rollbackVerified
        Write-I2eJson -Path $PointerPath -Value $pointer
        if ($serviceCreated -and $rollbackVerified) {
            Stop-I2eService | Out-Null
            Remove-I2eService
        }
        Write-I2eJson -Path $PointerPath -Value $pointer
    }
    catch {
        $cleanupError = $_.Exception
    }
}

if ($null -ne $cleanupError) { throw ('I2E cleanup failed closed: {0}' -f $cleanupError.Message) }
if ($null -ne $primaryError) { throw $primaryError }
