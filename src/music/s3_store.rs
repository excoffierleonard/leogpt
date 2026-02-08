//! S3-backed music storage for playback and listing.

use std::{sync::Arc, time::Duration};

use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client, presigning::PresigningConfig};
use log::{info, warn};
use tokio::sync::RwLock;

use crate::{config::MusicS3Config, error::Result};

use super::fuzzy_search::{find_song, list_songs};

#[derive(Clone, Debug)]
pub struct S3Entry {
    pub key: String,
    pub name: String,
}

#[derive(Debug)]
struct S3Cache {
    loaded: bool,
    entries: Vec<S3Entry>,
}

/// S3 music store with a one-time startup cache.
#[derive(Debug)]
pub struct S3MusicStore {
    client: Client,
    bucket: String,
    prefix: String,
    cache: RwLock<S3Cache>,
}

impl S3MusicStore {
    /// Build a new S3 music store from configuration and eagerly load the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the AWS config or credentials cannot be loaded.
    pub async fn from_config(config: &MusicS3Config) -> Result<Self> {
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .endpoint_url(config.endpoint.clone())
            .load()
            .await;

        let client = Client::new(&shared_config);

        let store = Self {
            client,
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
            cache: RwLock::new(S3Cache {
                loaded: false,
                entries: Vec::new(),
            }),
        };

        store.load_cache().await?;

        Ok(store)
    }

    /// Load the object list into memory. Intended to be called once at startup.
    ///
    /// # Errors
    ///
    /// Returns an error if listing objects from S3 fails.
    pub async fn load_cache(&self) -> Result<()> {
        let mut entries = Vec::new();
        let mut token: Option<String> = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&self.prefix);

            if let Some(ref token) = token {
                request = request.continuation_token(token);
            }

            let response = request.send().await?;

            if let Some(objects) = response.contents {
                for object in objects {
                    let Some(key) = object.key else {
                        continue;
                    };

                    if key.ends_with('/') {
                        continue;
                    }

                    let name = key.rsplit('/').next().unwrap_or(&key).to_string();
                    if name.starts_with('.') {
                        continue;
                    }

                    entries.push(S3Entry { key, name });
                }
            }

            if response.is_truncated == Some(true) {
                token = response.next_continuation_token;
                if token.is_none() {
                    warn!("S3 listing truncated but no continuation token provided");
                    break;
                }
            } else {
                break;
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let mut cache = self.cache.write().await;
        cache.entries = entries;
        cache.loaded = true;

        info!(
            "Loaded {} music objects from s3://{}/{}",
            cache.entries.len(),
            self.bucket,
            self.prefix
        );

        Ok(())
    }

    /// Find a song in the cached list using fuzzy matching.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache is not loaded.
    pub async fn find_song(&self, query: &str) -> Result<Option<S3Entry>> {
        let cache = self.cache.read().await;
        Ok(find_song(&cache.entries, query).cloned())
    }

    /// # Errors
    ///
    /// Returns an error if the cache is not loaded.
    pub async fn list_songs(&self, limit: usize) -> Result<Vec<String>> {
        let cache = self.cache.read().await;
        Ok(list_songs(&cache.entries, limit))
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
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(config)
            .await?;

        Ok(presigned.uri().to_string())
    }
}

/// Shared store handle for command usage.
pub type SharedS3MusicStore = Arc<S3MusicStore>;
