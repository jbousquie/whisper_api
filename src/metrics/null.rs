use crate::metrics::metrics::MetricsExporter;
use async_trait::async_trait;
/// Null exporter for testing or when metrics are disabled
pub struct NullExporter;

#[async_trait]
impl MetricsExporter for NullExporter {
    async fn increment(&self, _name: &str, _labels: &[(&str, &str)]) {
        // No-op
    }

    async fn set_gauge(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // No-op
    }

    async fn observe_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // No-op
    }

    async fn export(&self) -> Result<Vec<u8>, String> {
        Ok(vec![])
    }
}