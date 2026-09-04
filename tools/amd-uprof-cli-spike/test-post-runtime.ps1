[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'postprocess.ps1')

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) { throw $Message }
}

function New-SyntheticRun {
    param(
        [int]$ExitCode = 0,
        [bool]$Timeout = $false,
        [AllowNull()][string]$HarnessError = $null
    )

    [pscustomobject]@{
        process_started = $true
        timeout = $Timeout
        target_exit_signed = $ExitCode
        capture_complete = $true
        harness_error = $HarnessError
    }
}

function Invoke-SyntheticCase {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Run
    )

    Invoke-AmdCliPostRuntimePipeline -SessionDirectory $Root -Run $Run
}

$wrapperPath = Join-Path $PSScriptRoot 'run-admin-amd-cli-spike.ps1'
$wrapperText = [System.IO.File]::ReadAllText($wrapperPath)
Assert-True -Condition ($wrapperText -notmatch 'Parse-PackagePowerCsv\s+-Path\s+\(if') -Message 'old if-as-argument-expression syntax is still present'
$postprocessPath = Join-Path $PSScriptRoot 'postprocess.ps1'
$postprocessText = [System.IO.File]::ReadAllText($postprocessPath)
Assert-True -Condition ($postprocessText -notmatch 'Parse-PackagePowerCsv\s+-Path\s+\(if') -Message 'old if-as-argument-expression syntax is still present in postprocess helper'
Assert-True -Condition ($postprocessText -match '\$csvPath\s*=\s*\$null') -Message 'explicit CSV path resolution is missing from postprocess helper'

$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('resource-timeline-amd-postprocess-' + [guid]::NewGuid().ToString('N'))
$sessionRoot = Join-Path $testRoot 'timechart-output'
[void](New-Item -ItemType Directory -Path $sessionRoot -Force)
$fixturePath = Join-Path $PSScriptRoot 'test-fixtures\package-power.csv'
$fixtureText = [System.IO.File]::ReadAllText($fixturePath)
$summaryPath = Join-Path $testRoot 'synthetic-summary.json'

try {
    [System.IO.File]::WriteAllText((Join-Path $sessionRoot 'timechart.csv'), $fixtureText)
    [System.IO.File]::WriteAllBytes((Join-Path $sessionRoot 'session.uprof'), [byte[]](0x01, 0x02, 0x03))
    $pass = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun)
    Assert-True -Condition ($pass.csv_path_resolution -eq 'PASS') -Message 'CSV path resolution failed'
    Assert-True -Condition $pass.parser_invoked -Message 'parser was not invoked'
    Assert-True -Condition ($pass.parsed_package_power.status -eq 'PASS') -Message 'synthetic parser did not pass'
    Assert-True -Condition ($pass.parsed_package_power.sample_count -eq 3) -Message 'synthetic sample count mismatch'
    Assert-True -Condition ($pass.qualification -eq 'PASS') -Message 'synthetic pass classification failed'

    $summary = [pscustomobject]@{
        csv_path_resolution = $pass.csv_path_resolution
        parser_invoked = $pass.parser_invoked
        qualification = $pass.qualification
    }
    [System.IO.File]::WriteAllText($summaryPath, ($summary | ConvertTo-Json -Depth 10))
    Assert-True -Condition (Test-Path -LiteralPath $summaryPath -PathType Leaf) -Message 'synthetic summary was not written'
    Assert-True -Condition ((Get-Item -LiteralPath $summaryPath).Length -gt 0) -Message 'synthetic summary is empty'

    Remove-Item -LiteralPath (Join-Path $sessionRoot 'timechart.csv') -Force
    $missingCsv = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun)
    Assert-True -Condition ($missingCsv.csv_path_resolution -eq 'NOT_FOUND') -Message 'missing CSV was not classified'
    Assert-True -Condition ($missingCsv.parser_invoked -and $missingCsv.parsed_package_power.status -eq 'NOT_FOUND') -Message 'missing CSV parser path failed'
    Assert-True -Condition ($missingCsv.qualification -eq 'OUTPUT_ARTIFACT_MISSING') -Message 'missing CSV classification failed'

    [System.IO.File]::WriteAllText((Join-Path $sessionRoot 'timechart.csv'), $fixtureText)
    Remove-Item -LiteralPath (Join-Path $sessionRoot 'session.uprof') -Force
    $missingUprof = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun)
    Assert-True -Condition ($missingUprof.parsed_package_power.status -eq 'PASS') -Message 'CSV should still parse without UPROF'
    Assert-True -Condition ($missingUprof.qualification -eq 'OUTPUT_ARTIFACT_MISSING') -Message 'missing UPROF classification failed'

    [System.IO.File]::WriteAllBytes((Join-Path $sessionRoot 'session.uprof'), [byte[]](0x01))
    [System.IO.File]::WriteAllText((Join-Path $sessionRoot 'timechart.csv'), 'not a timechart report')
    $parseFail = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun)
    Assert-True -Condition ($parseFail.parsed_package_power.status -eq 'PARSE_FAILED') -Message 'parse failure was not retained'
    Assert-True -Condition ($parseFail.qualification -eq 'PARSE_FAILED') -Message 'parse failure classification failed'

    [System.IO.File]::WriteAllText((Join-Path $sessionRoot 'timechart.csv'), $fixtureText)
    $targetFail = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun -ExitCode -1)
    Assert-True -Condition ($targetFail.qualification -eq 'TARGET_FAILED') -Message 'negative target exit classification failed'
    $timeout = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun -Timeout $true)
    Assert-True -Condition ($timeout.qualification -eq 'TIMEOUT') -Message 'timeout classification failed'
    $harnessFail = Invoke-SyntheticCase -Root $sessionRoot -Run (New-SyntheticRun -HarnessError 'synthetic harness failure')
    Assert-True -Condition ($harnessFail.qualification -eq 'HARNESS_FAILED') -Message 'harness failure classification failed'

    Write-Output 'CSV_PATH_RESOLUTION=PASS'
    Write-Output 'PARSER_INVOKED=PASS'
    Write-Output 'SUMMARY_WRITTEN=PASS'
    Write-Output 'FULL_POST_RUNTIME_SYNTHETIC_PATH=PASS'
    Write-Output 'AMD_RUNTIME_EXECUTED=false'
    Write-Output 'SERVICE_REGISTERED=false'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
