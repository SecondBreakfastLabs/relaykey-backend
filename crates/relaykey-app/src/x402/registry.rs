use std::{collections::HashMap, sync::Arc};
use super::provider::PaymentProvider;

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn PaymentProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(), 
        }
    }

    pub fn register(
        mut self, 
        name: impl Into<String>, 
        provider: Arc<dyn PaymentProvider>, 
    ) -> Self {
        self.providers.insert(name.into(), provider); 
        self
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn PaymentProvider>> {
        self.providers.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}