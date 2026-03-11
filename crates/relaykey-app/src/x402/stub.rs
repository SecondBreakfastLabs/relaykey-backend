use async_trait::async_trait;

use super::provider::{PaymentProvider, VerifyInput, VerifyOutput};

#[derive(Debug, Clone, Default)]
pub struct StubProvider;

#[async_trait]
impl PaymentProvider for StubProvider {
    fn name(&self) -> &'static str {
        "stub"
    }

    async fn verify(&self, _input: VerifyInput<'_>) -> anyhow::Result<VerifyOutput> {
        // Placeholder for a real facilitator verify call
        Ok(VerifyOutput {
            verified: false,
            reason: Some("stub provider does not verify payments".to_string()),
        })
    }
}
