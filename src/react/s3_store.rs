//! S3-backed meme storage for reaction images.

use log::info;
use tokio::sync::RwLock;

use crate::{
    config::S3Config,
    error::Result,
    s3_index::{S3Cache, S3Index},
};

/// S3 meme store with a one-time startup cache.
#[derive(Debug)]
pub struct S3MemeStore {
    index: S3Index,
    public_base_url: String,
}

impl S3MemeStore {
    /// Build a new S3 meme store from configuration and eagerly load the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the AWS config or credentials cannot be loaded.
    pub async fn from_config(config: &S3Config) -> Result<Self> {
        let index = S3Index::new(config).await?;
        let count = index.cache().read().await.entries.len();
        info!("Loaded {count} meme objects");
        Ok(Self {
            index,
            public_base_url: config.public_base_url.clone(),
        })
    }

    #[must_use]
    pub fn public_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_base_url.trim_end_matches('/'), key)
    }

    pub(crate) fn cache(&self) -> &RwLock<S3Cache> {
        self.index.cache()
    }
}
