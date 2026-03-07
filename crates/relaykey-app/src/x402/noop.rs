use async_trait::async_trait;

use super::provider::{
    PaymentProvider, 
    VerifyInput,
    VerifyOutput,
}; 

#[derive(Debug, Clone, Default)]
pub struct NoopProvider;

#[async_trait]
impl PaymentProvider for NoopProvider{
    fn name(&self) -> &'static str {
        "noop"
    }

    async fn verify(&self, input: VerifyInput<'_>) -> anyhow::Result<VerifyOutput> {
        // Dev Behavior: 
        // If caller supplied either header, treat as paid for now. 
        let ok = input.payment_id.is_some() || input.payment_token.is_some(); 
        Ok(VerifyOutput {
            verified: ok, 
            reason: if ok { None } else { Some("missing payment proof".to_string()) }, 
        })
    }
}