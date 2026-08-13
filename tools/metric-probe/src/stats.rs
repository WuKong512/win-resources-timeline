use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Distribution {
    pub count: usize,
    pub min: Option<f64>,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
}

impl Distribution {
    pub fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self {
                count: 0,
                min: None,
                p50: None,
                p95: None,
                max: None,
                mean: None,
            };
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let sum: f64 = sorted.iter().sum();
        Self {
            count: sorted.len(),
            min: sorted.first().copied(),
            p50: percentile(values, 0.50),
            p95: percentile(values, 0.95),
            max: sorted.last().copied(),
            mean: Some(sum / sorted.len() as f64),
        }
    }
}

pub fn percentile(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() || !(0.0..=1.0).contains(&percentile) {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    percentile_sorted(&sorted, percentile)
}

fn percentile_sorted(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let position = percentile * (values.len().saturating_sub(1) as f64);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return values.get(lower).copied();
    }
    let weight = position - lower as f64;
    Some(values[lower] + (values[upper] - values[lower]) * weight)
}

#[cfg(test)]
mod tests {
    use super::{percentile, Distribution};

    #[test]
    fn computes_interpolated_percentiles() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), Some(2.5));
        let p95 = percentile(&[1.0, 2.0, 3.0, 4.0], 0.95).unwrap();
        assert!((p95 - 3.85).abs() < 1e-12);
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(percentile(&[1.0], 1.1), None);
    }

    #[test]
    fn distribution_handles_empty_and_nonempty_values() {
        assert_eq!(Distribution::from_values(&[]).count, 0);
        let distribution = Distribution::from_values(&[1.0, 2.0, 5.0]);
        assert_eq!(distribution.count, 3);
        assert_eq!(distribution.p50, Some(2.0));
        assert_eq!(distribution.max, Some(5.0));
    }
}
