pub fn percentile(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let rank = percentile.clamp(0.0, 1.0) * (sorted.len().saturating_sub(1) as f64);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        return sorted.get(lower).copied();
    }
    let weight = rank - lower as f64;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * weight)
}

pub fn average(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub fn max(values: &[f64]) -> Option<f64> {
    values.iter().copied().reduce(f64::max)
}

pub fn linear_slope_per_hour(samples: &[(u64, f64)]) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let mean_x = samples.iter().map(|(x, _)| *x as f64).sum::<f64>() / samples.len() as f64;
    let mean_y = samples.iter().map(|(_, y)| *y).sum::<f64>() / samples.len() as f64;
    let denominator = samples
        .iter()
        .map(|(x, _)| {
            let delta = *x as f64 - mean_x;
            delta * delta
        })
        .sum::<f64>();
    if denominator <= f64::EPSILON {
        return None;
    }
    let numerator = samples
        .iter()
        .map(|(x, y)| (*x as f64 - mean_x) * (*y - mean_y))
        .sum::<f64>();
    Some(numerator / denominator * 3_600_000.0)
}

#[cfg(test)]
mod tests {
    use super::{average, linear_slope_per_hour, percentile};

    #[test]
    fn percentile_interpolates_between_samples() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), Some(2.5));
        assert_eq!(percentile(&[], 0.95), None);
    }

    #[test]
    fn slope_is_reported_in_units_per_hour() {
        assert_eq!(
            linear_slope_per_hour(&[(0, 10.0), (3_600_000, 20.0)]),
            Some(10.0)
        );
        assert_eq!(average(&[1.0, 3.0]), Some(2.0));
    }
}
