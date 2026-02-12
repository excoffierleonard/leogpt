//! Shared S3 object indexing for startup caches.

use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::Client;
use tokio::sync::RwLock;

use crate::{config::S3Config, error::Result};

#[derive(Clone, Debug)]
pub struct S3Entry {
    pub key: String,
    pub name: String,
}

#[derive(Debug)]
pub struct S3Cache {
    pub entries: Vec<S3Entry>,
}

#[derive(Debug)]
pub struct S3Index {
    client: Client,
    bucket: String,
    prefix: String,
    cache: RwLock<S3Cache>,
}

impl S3Index {
    /// Build a new S3 index from configuration and eagerly load the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the AWS config or credentials cannot be loaded.
    pub async fn new(config: &S3Config) -> Result<Self> {
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .endpoint_url(config.endpoint.clone())
            .load()
            .await;

        let client = Client::new(&shared_config);

        let index = Self {
            client,
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
            cache: RwLock::new(S3Cache {
                entries: Vec::new(),
            }),
        };

        index.load_cache().await?;

        Ok(index)
    }

    /// Load the object list into memory. Intended to be called once at startup.
    ///
    /// # Errors
    ///
    /// Returns an error if listing objects from S3 fails.
    pub async fn load_cache(&self) -> Result<()> {
        let mut entries = Vec::new();
        let mut pages = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&self.prefix)
            .into_paginator()
            .send();

        while let Some(page) = pages.next().await {
            let response = page?;
            entries.extend(
                response
                    .contents()
                    .iter()
                    .filter_map(|object| object.key())
                    .filter(|key| {
                        !key.ends_with('/')
                            && !key.rsplit('/').next().unwrap_or(key).starts_with('.')
                    })
                    .map(|key| S3Entry {
                        key: key.to_string(),
                        name: key.rsplit('/').next().unwrap_or(key).to_string(),
                    }),
            );
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let mut cache = self.cache.write().await;
        cache.entries = entries;

        Ok(())
    }

    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }

    #[must_use]
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    #[must_use]
    pub fn cache(&self) -> &RwLock<S3Cache> {
        &self.cache
    }
}
