//! S3-backed music storage for playback and listing.

use std::time::Duration;

use aws_sdk_s3::presigning::PresigningConfig;
use log::info;
use tokio::sync::RwLock;

use crate::{
    config::S3Config,
    error::Result,
    s3_index::{S3Cache, S3Index},
};

pub use crate::s3_index::S3Entry;

/// S3 music store with a one-time startup cache.
#[derive(Debug)]
pub struct S3MusicStore {
    index: S3Index,
}

impl S3MusicStore {
    /// Build a new S3 music store from configuration and eagerly load the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the AWS config or credentials cannot be loaded.
    pub async fn from_config(config: &S3Config) -> Result<Self> {
        let index = S3Index::new(config).await?;
        let count = index.cache().read().await.entries.len();
        info!("Loaded {count} music objects");
        Ok(Self { index })
    }

    /// Create a presigned URL for streaming.
    ///
    /// # Errors
    ///
    /// Returns an error if presigning fails.
    pub async fn presigned_url(&self, key: &str) -> Result<String> {
        let config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(3600))
            .build()?;

        let presigned = self
            .index
            .client()
            .get_object()
            .bucket(self.index.bucket())
            .key(key)
            .presigned(config)
            .await?;

        Ok(presigned.uri().to_string())
    }

    pub(crate) fn cache(&self) -> &RwLock<S3Cache> {
        self.index.cache()
    }
}
