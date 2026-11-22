use aws_sdk_s3::{
    config::Region,
    primitives::ByteStream,
    Client,
};
use std::{env, path::{Component, Path, PathBuf}};

#[derive(Clone)]
enum Backend {
    Local { directory: PathBuf },
    S3 { client: Client, bucket: String },
}

#[derive(Clone)]
pub struct StorageService {
    backend: Backend,
}

impl StorageService {
    pub async fn from_env() -> Result<Self, std::io::Error> {
        let mode = env::var("CAMPUSFORGE_STORAGE_MODE").unwrap_or_else(|_| "local".to_string());
        match mode.as_str() {
            "local" => {
                let directory = env::var("CAMPUSFORGE_UPLOAD_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("server/uploads"));
                tokio::fs::create_dir_all(&directory).await?;
                tracing::warn!(
                    path = %directory.display(),
                    "Using local development file storage; configure S3 storage before production"
                );
                Ok(Self { backend: Backend::Local { directory } })
            }
            "s3" => {
                let bucket = env::var("CAMPUSFORGE_S3_BUCKET")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "CAMPUSFORGE_S3_BUCKET is required when storage mode is s3",
                    ))?;
                let region = env::var("CAMPUSFORGE_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
                let shared = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .region(Region::new(region))
                    .load()
                    .await;
                let mut config = aws_sdk_s3::config::Builder::from(&shared);
                if let Ok(endpoint) = env::var("CAMPUSFORGE_S3_ENDPOINT_URL") {
                    config = config.endpoint_url(endpoint).force_path_style(true);
                }
                let client = Client::from_conf(config.build());
                tracing::info!(bucket = %bucket, "S3-compatible private object storage configured");
                Ok(Self { backend: Backend::S3 { client, bucket } })
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "CAMPUSFORGE_STORAGE_MODE must be local or s3",
            )),
        }
    }

    fn validate_key(key: &str) -> Result<(), String> {
        if key.is_empty()
            || key.len() > 180
            || !Path::new(key).components().all(|component| matches!(component, Component::Normal(_)))
            || !key.chars().all(|character| character.is_ascii_alphanumeric() || "-_./".contains(character))
        {
            return Err("Invalid server-generated object key".to_string());
        }
        Ok(())
    }

    pub async fn put(&self, key: &str, bytes: &[u8], content_type: &str) -> Result<(), String> {
        Self::validate_key(key)?;
        match &self.backend {
            Backend::Local { directory } => {
                let path = directory.join(key);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|error| error.to_string())?;
                }
                tokio::fs::write(path, bytes).await.map_err(|error| error.to_string())
            }
            Backend::S3 { client, bucket } => {
                client.put_object()
                    .bucket(bucket)
                    .key(key)
                    .content_type(content_type)
                    .body(ByteStream::from(bytes.to_vec()))
                    .send()
                    .await
                    .map_err(|error| format!("Object storage upload failed: {error}"))?;
                Ok(())
            }
        }
    }

    pub async fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        Self::validate_key(key)?;
        match &self.backend {
            Backend::Local { directory } => {
                tokio::fs::read(directory.join(key)).await.map_err(|error| error.to_string())
            }
            Backend::S3 { client, bucket } => {
                let output = client.get_object().bucket(bucket).key(key).send().await
                    .map_err(|error| format!("Object storage download failed: {error}"))?;
                let body = output.body.collect().await
                    .map_err(|error| format!("Object storage response could not be read: {error}"))?;
                Ok(body.into_bytes().to_vec())
            }
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), String> {
        Self::validate_key(key)?;
        match &self.backend {
            Backend::Local { directory } => {
                tokio::fs::remove_file(directory.join(key)).await.map_err(|error| error.to_string())
            }
            Backend::S3 { client, bucket } => {
                client.delete_object().bucket(bucket).key(key).send().await
                    .map_err(|error| format!("Object storage delete failed: {error}"))?;
                Ok(())
            }
        }
    }

}
