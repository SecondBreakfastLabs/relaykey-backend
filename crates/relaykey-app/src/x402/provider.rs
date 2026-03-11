use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct VerifyInput<'a> {
    pub payment_id: Option<&'a str>,
    pub payment_token: Option<&'a str>,
    pub amount: &'a str,
    pub currency: &'a str,
    pub recipient: &'a str,
    pub facilitator_url: &'a str,
}

#[derive(Debug, Clone)]
pub struct VerifyOutput {
    pub verified: bool,
    pub reason: Option<String>,
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn verify(&self, input: VerifyInput<'_>) -> anyhow::Result<VerifyOutput>;
}
