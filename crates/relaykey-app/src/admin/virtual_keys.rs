use axum::{
    Extension,
    Json,
};
use std::sync::Arc;

use crate::state::AppState;

use relaykey_db::queries::admin::{
    insert_virtual_key,
    list_virtual_keys
};

use super::keygen::generate_virtual_key;
use relaykey_core::crypto::key_hash::hash_virtual_key;
