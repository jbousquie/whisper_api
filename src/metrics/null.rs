/// Null exporter for testing or when metrics are disabled
/// This exporter does not collect or export any metrics, effectively acting as a no-op.
///
use crate::metrics::metrics::MetricsExporter;
use async_trait::async_trait;
pub struct NullExporter;

#[async_trait]
impl MetricsExporter for NullExporter {
    async fn increment(&self, _name: &str, _labels: &[(&str, &str)]) {
        // NOP
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
