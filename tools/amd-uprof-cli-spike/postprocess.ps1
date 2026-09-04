Set-StrictMode -Version Latest

function Normalize-Header {
    param([Parameter(Mandatory = $true)][string]$Value)
    ($Value.Trim().ToLowerInvariant() -replace '[ _]', '-')
}

function Parse-PackagePowerCsv {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return [pscustomobject]@{ status = 'NOT_FOUND'; error = 'timechart.csv was not found'; sample_count = 0; samples = @() }
    }
    $lines = [System.IO.File]::ReadAllLines($Path)
    $recordsIndex = -1
    for ($index = 0; $index -lt $lines.Length; $index++) {
        if ($lines[$index].Trim() -ieq 'PROFILE RECORDS') { $recordsIndex = $index; break }
    }
    if ($recordsIndex -lt 0) {
        return [pscustomobject]@{ status = 'PARSE_FAILED'; error = 'PROFILE RECORDS section missing'; sample_count = 0; samples = @() }
    }
    $headerIndex = $recordsIndex + 1
    while ($headerIndex -lt $lines.Length -and [string]::IsNullOrWhiteSpace($lines[$headerIndex])) { $headerIndex++ }
    if ($headerIndex -ge $lines.Length) {
        return [pscustomobject]@{ status = 'PARSE_FAILED'; error = 'record header missing'; sample_count = 0; samples = @() }
    }
    $headers = $lines[$headerIndex].Split(',') | ForEach-Object { Normalize-Header -Value $_ }
    $powerIndex = -1
    for ($index = 0; $index -lt $headers.Count; $index++) {
        if ($headers[$index] -eq 'socket0-package-power' -or $headers[$index].Contains('package-power')) {
            $powerIndex = $index
            break
        }
    }
    if ($powerIndex -lt 0) {
        return [pscustomobject]@{ status = 'COUNTER_UNAVAILABLE'; error = 'package-power column missing'; sample_count = 0; samples = @() }
    }
    $timestampIndex = [array]::IndexOf($headers, 'timestamp')
    $recordIdIndex = [array]::IndexOf($headers, 'record-id')
    $samples = @()
    for ($index = $headerIndex + 1; $index -lt $lines.Length; $index++) {
        if ([string]::IsNullOrWhiteSpace($lines[$index])) { continue }
        $fields = $lines[$index].Split(',')
        if ($fields.Count -ne $headers.Count) {
            return [pscustomobject]@{ status = 'PARSE_FAILED'; error = "record column count mismatch at line $($index + 1)"; sample_count = 0; samples = @() }
        }
        $rawValue = $fields[$powerIndex].Trim()
        $value = 0.0
        if (-not [double]::TryParse($rawValue, [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$value)) {
            return [pscustomobject]@{ status = 'PARSE_FAILED'; error = "package-power value is not invariant numeric at line $($index + 1)"; sample_count = 0; samples = @() }
        }
        if ([double]::IsNaN($value) -or [double]::IsInfinity($value) -or $value -lt 0) {
            return [pscustomobject]@{ status = 'PARSE_FAILED'; error = "package-power value is invalid at line $($index + 1)"; sample_count = 0; samples = @() }
        }
        $samples += [pscustomobject]@{
            record_id = if ($recordIdIndex -ge 0) { $fields[$recordIdIndex].Trim() } else { $null }
            timestamp = if ($timestampIndex -ge 0) { $fields[$timestampIndex].Trim() } else { $null }
            raw_value = $rawValue
            value_watts = $value
            unit = 'W'
        }
    }
    if ($samples.Count -eq 0) {
        return [pscustomobject]@{ status = 'PARSE_FAILED'; error = 'no package-power records'; sample_count = 0; samples = @() }
    }
    [pscustomobject]@{ status = 'PASS'; error = $null; sample_count = $samples.Count; samples = $samples }
}

function Get-OutputInventory {
    param([Parameter(Mandatory = $true)][string]$Root)
    @(Get-ChildItem -LiteralPath $Root -File -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
        [pscustomobject]@{
            path = $_.FullName
            relative_path = $_.FullName.Substring($Root.TrimEnd('\').Length).TrimStart('\')
            size_bytes = [long]$_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToUpperInvariant()
        }
    })
}

function Get-AmdCliQualification {
    param(
        [Parameter(Mandatory = $true)]$Run,
        [string]$CsvPath,
        [string]$UprofPath,
        [Parameter(Mandatory = $true)]$Parsed
    )

    $hasCsv = -not [string]::IsNullOrWhiteSpace($CsvPath)
    $hasUprof = -not [string]::IsNullOrWhiteSpace($UprofPath)
    if ($Run.harness_error) { return 'HARNESS_FAILED' }
    if ($Run.timeout) { return 'TIMEOUT' }
    if ($Run.process_started -and -not $Run.timeout -and $Run.target_exit_signed -eq 0 -and
        $Run.capture_complete -and $hasCsv -and $hasUprof -and $Parsed.status -eq 'PASS') {
        return 'PASS'
    }
    if ($Parsed.status -eq 'PARSE_FAILED') { return 'PARSE_FAILED' }
    if ($Parsed.status -eq 'NOT_FOUND' -or -not $hasCsv -or -not $hasUprof) {
        return 'OUTPUT_ARTIFACT_MISSING'
    }
    if ($Parsed.status -eq 'COUNTER_UNAVAILABLE') { return 'COUNTER_UNAVAILABLE' }
    'TARGET_FAILED'
}

function Invoke-AmdCliPostRuntimePipeline {
    param(
        [Parameter(Mandatory = $true)][string]$SessionDirectory,
        [Parameter(Mandatory = $true)]$Run
    )

    $csvFile = Get-ChildItem -LiteralPath $SessionDirectory -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -ieq 'timechart.csv' } | Select-Object -First 1
    $uprofFile = Get-ChildItem -LiteralPath $SessionDirectory -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -ieq 'session.uprof' } | Select-Object -First 1

    # Keep this explicit.  PowerShell does not accept if as a command inside
    # a parenthesized argument expression on the supported Windows host.
    $csvPath = $null
    if ($null -ne $csvFile) {
        $csvPath = $csvFile.FullName
    }

    $parserInvoked = $true
    try {
        $parsed = Parse-PackagePowerCsv -Path $csvPath
    } catch {
        $parsed = [pscustomobject]@{
            status = 'PARSE_FAILED'
            error = $_.Exception.Message
            sample_count = 0
            samples = @()
        }
    }
    $uprofPath = if ($null -ne $uprofFile) { $uprofFile.FullName } else { $null }
    $qualification = Get-AmdCliQualification -Run $Run -CsvPath $csvPath -UprofPath $uprofPath -Parsed $parsed
    $outputArtifacts = @(Get-OutputInventory -Root $SessionDirectory)

    [pscustomobject]@{
        csv_path_resolution = if ($null -ne $csvPath) { 'PASS' } else { 'NOT_FOUND' }
        timechart_csv_path = $csvPath
        session_uprof_path = $uprofPath
        parser_invoked = $parserInvoked
        parsed_package_power = $parsed
        output_artifacts = [object[]]$outputArtifacts
        qualification = $qualification
    }
}
