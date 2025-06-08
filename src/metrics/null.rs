use crate::metrics::error::MetricsError;
/// Null exporter for testing or when metrics are disabled
/// This exporter does not collect or export any metrics, effectively acting as a no-op.
///
use crate::metrics::metrics::MetricsExporter;
use async_trait::async_trait;
pub struct NullExporter;

#[async_trait]
impl MetricsExporter for NullExporter {
    async fn increment(&self, _name: &str, _labels: &[(&str, &str)]) -> Result<(), MetricsError> {
        // NOP
        Ok(())
    }
    async fn set_gauge(
        &self,
        _name: &str,
        _value: f64,
        _labels: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        // No-op
        Ok(())
    }
    async fn observe_histogram(
        &self,
        _name: &str,
        _value: f64,
        _labels: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        // No-op
        Ok(())
    }
    async fn export(&self) -> Result<Vec<u8>, MetricsError> {
        Ok(vec![])
    }
}
