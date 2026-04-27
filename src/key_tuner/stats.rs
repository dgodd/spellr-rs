pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn mean_by<T, F: Fn(&T) -> f64>(values: &[T], f: F) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(f).sum::<f64>() / values.len() as f64
}

pub fn max_by<T, F: Fn(&T) -> f64>(values: &[T], f: F) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values
        .iter()
        .map(f)
        .fold(f64::NEG_INFINITY, f64::max)
}

pub fn variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let m = mean(values);
    values.iter().map(|v| (m - v).powi(2)).sum::<f64>() / values.len() as f64
}

pub fn variance_by<T, F: Fn(&T) -> f64>(values: &[T], f: F) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mapped: Vec<f64> = values.iter().map(f).collect();
    variance(&mapped)
}

pub fn gaussian_probability(value: f64, standard_deviation: f64, mean: f64, variance: f64) -> f64 {
    if standard_deviation == 0.0 {
        return if mean == value { 1.0 } else { 0.0 };
    }
    let exp = -((value - mean).powi(2)) / (2.0 * variance);
    (1.0 / (2.0 * std::f64::consts::PI * variance).sqrt()) * std::f64::consts::E.powf(exp)
}
