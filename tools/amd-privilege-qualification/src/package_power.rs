//! Header-driven package-power parsing used by the future broker result path.
//!
//! The shape intentionally follows the already validated AMD CLI parser contract: it selects
//! the package-power column by header, requires unit W, rejects missing/non-finite/negative
//! values, preserves clock timestamps, and never invents a sample.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackagePowerSample {
    pub timestamp: String,
    pub clock_millis: u64,
    pub value_watts: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackagePowerParseResult {
    pub counter_name: String,
    pub unit: String,
    pub samples: Vec<PackagePowerSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceAssessment {
    pub status: String,
    pub expected_interval_ms: u32,
    pub deltas_ms: Vec<u64>,
    pub min_ms: Option<u64>,
    pub max_ms: Option<u64>,
    pub mean_ms: Option<u64>,
}

pub fn parse_package_power_csv(input: &str) -> Result<PackagePowerParseResult, String> {
    if input.trim().is_empty() {
        return Err("CLI output is empty".to_owned());
    }
    let lines: Vec<&str> = input.lines().collect();
    let counters_marker = find_line(&lines, "PROFILED COUNTERS")
        .ok_or_else(|| "CLI output is missing PROFILED COUNTERS".to_owned())?;
    let records_marker = find_line(&lines, "PROFILE RECORDS")
        .ok_or_else(|| "CLI output is missing PROFILE RECORDS".to_owned())?;
    if records_marker <= counters_marker {
        return Err("PROFILE RECORDS precedes PROFILED COUNTERS".to_owned());
    }
    let counter_header_line = next_nonempty_line(&lines, counters_marker + 1)
        .ok_or_else(|| "counter header is missing".to_owned())?;
    let counter_headers = parse_csv_line(lines[counter_header_line])?;
    let unit_index =
        column(&counter_headers, "unit").ok_or_else(|| "counter unit is missing".to_owned())?;
    let name_index =
        column(&counter_headers, "name").ok_or_else(|| "counter name is missing".to_owned())?;
    let mut package_counter: Option<(String, String)> = None;
    for line in &lines[counter_header_line + 1..records_marker] {
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line)?;
        if fields.len() != counter_headers.len() {
            return Err("counter row has the wrong number of columns".to_owned());
        }
        let name = fields[name_index].trim().to_owned();
        if normalize_field(&name) == "socket0-package-power"
            || normalize_field(&name).contains("package-power")
        {
            let unit = fields[unit_index].trim().to_owned();
            if !unit.eq_ignore_ascii_case("W") {
                return Err(format!("package-power unit is unsupported: {unit}"));
            }
            package_counter = Some((name, unit));
            break;
        }
    }
    let (counter_name, unit) =
        package_counter.ok_or_else(|| "package-power counter is missing".to_owned())?;
    let record_header_line = next_nonempty_line(&lines, records_marker + 1)
        .ok_or_else(|| "record header is missing".to_owned())?;
    let record_headers = parse_csv_line(lines[record_header_line])?;
    let timestamp_index = record_headers
        .iter()
        .position(|header| normalize_field(header) == "timestamp")
        .ok_or_else(|| "record timestamp is missing".to_owned())?;
    let power_index = record_headers
        .iter()
        .position(|header| normalize_field(header) == normalize_field(&counter_name))
        .or_else(|| {
            record_headers
                .iter()
                .position(|header| normalize_field(header).contains("package-power"))
        })
        .ok_or_else(|| "record package-power column is missing".to_owned())?;
    let mut timestamps = BTreeSet::new();
    let mut samples = Vec::new();
    for line in &lines[record_header_line + 1..] {
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line)?;
        if fields.len() != record_headers.len() {
            return Err("record row has the wrong number of columns".to_owned());
        }
        let timestamp = fields[timestamp_index].trim().to_owned();
        let clock_millis = parse_timestamp(&timestamp)
            .ok_or_else(|| format!("timestamp is malformed: {timestamp}"))?;
        if !timestamps.insert(clock_millis) {
            return Err(format!("timestamp is duplicated: {timestamp}"));
        }
        let raw_value = fields[power_index].trim();
        if raw_value.is_empty()
            || matches!(
                raw_value.to_ascii_lowercase().as_str(),
                "-" | "--" | "na" | "n/a" | "null"
            )
        {
            return Err("package-power value is missing".to_owned());
        }
        let value_watts = raw_value
            .parse::<f64>()
            .map_err(|_| format!("package-power value is malformed: {raw_value}"))?;
        if !value_watts.is_finite() || value_watts < 0.0 {
            return Err("package-power value must be finite and non-negative".to_owned());
        }
        samples.push(PackagePowerSample {
            timestamp,
            clock_millis,
            value_watts,
        });
    }
    if samples.is_empty() {
        return Err("PROFILE RECORDS contains no package-power samples".to_owned());
    }
    Ok(PackagePowerParseResult {
        counter_name,
        unit,
        samples,
    })
}

pub fn assess_cadence(
    samples: &[PackagePowerSample],
    expected_interval_ms: u32,
) -> CadenceAssessment {
    let mut deltas_ms = Vec::new();
    let day = 86_400_000_u64;
    for pair in samples.windows(2) {
        let raw = pair[1].clock_millis as i128 - pair[0].clock_millis as i128;
        let delta = if raw < 0 {
            (raw + day as i128) as u64
        } else {
            raw as u64
        };
        deltas_ms.push(delta);
    }
    let min_ms = deltas_ms.iter().copied().min();
    let max_ms = deltas_ms.iter().copied().max();
    let mean_ms =
        (!deltas_ms.is_empty()).then(|| deltas_ms.iter().sum::<u64>() / deltas_ms.len() as u64);
    let tolerance = u64::from(expected_interval_ms).max(100) / 10;
    let status = if !deltas_ms.is_empty()
        && deltas_ms
            .iter()
            .all(|delta| delta.abs_diff(u64::from(expected_interval_ms)) <= tolerance)
    {
        "PASS"
    } else {
        "INCONCLUSIVE"
    };
    CadenceAssessment {
        status: status.to_owned(),
        expected_interval_ms,
        deltas_ms,
        min_ms,
        max_ms,
        mean_ms,
    }
}

fn find_line(lines: &[&str], marker: &str) -> Option<usize> {
    lines.iter().position(|line| {
        line.trim()
            .trim_start_matches('\u{feff}')
            .eq_ignore_ascii_case(marker)
    })
}

fn next_nonempty_line(lines: &[&str], start: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, line)| !line.trim().is_empty())
        .map(|(index, _)| index)
}

fn column(headers: &[String], name: &str) -> Option<usize> {
    headers
        .iter()
        .position(|header| normalize_field(header) == name)
}

fn normalize_field(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
}

fn parse_timestamp(value: &str) -> Option<u64> {
    let normalized = value.trim().replace('.', ":");
    let parts: Vec<&str> = normalized.split(':').collect();
    if parts.len() != 4 {
        return None;
    }
    let hour = parts[0].parse::<u64>().ok()?;
    let minute = parts[1].parse::<u64>().ok()?;
    let second = parts[2].parse::<u64>().ok()?;
    let millis = parts[3].parse::<u64>().ok()?;
    (hour < 24 && minute < 60 && second < 60 && millis < 1_000)
        .then_some((((hour * 60) + minute) * 60 + second) * 1_000 + millis)
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                field.push('"');
                characters.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            _ => field.push(character),
        }
    }
    if quoted {
        return Err("unterminated CSV quote".to_owned());
    }
    fields.push(field);
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../amd-uprof-cli-spike/test-fixtures/package-power.csv");

    #[test]
    fn parser_matches_existing_package_power_fixture_contract() {
        let parsed = parse_package_power_csv(FIXTURE).unwrap();
        assert_eq!(parsed.samples.len(), 3);
        assert_eq!(parsed.samples[0].value_watts, 49.44);
        assert_eq!(parsed.unit, "W");
        assert_eq!(assess_cadence(&parsed.samples, 1_000).status, "PASS");
    }

    #[test]
    fn parser_rejects_missing_negative_duplicate_and_wrong_unit() {
        assert!(parse_package_power_csv(&FIXTURE.replace("49.44", "N/A")).is_err());
        assert!(parse_package_power_csv(&FIXTURE.replace("49.44", "-1")).is_err());
        assert!(
            parse_package_power_csv(&FIXTURE.replace("2,11:18:23:646", "2,11:18:22:646")).is_err()
        );
        assert!(parse_package_power_csv(&FIXTURE.replace(
            "48.,socket0-package-power,Power,W,",
            "48.,socket0-package-power,Power,kW,"
        ))
        .is_err());
    }
}
