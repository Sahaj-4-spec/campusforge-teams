use hmac::{Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HOST},
    redirect::Policy, Client, Method,
};
use sha2::{Digest, Sha256};
use std::{env, path::{Component, Path, PathBuf}};
use tokio_util::io::ReaderStream;
use url::Url;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct S3Backend {
    client: Client,
    bucket: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
    endpoint_url: Option<String>,
}

#[derive(Clone)]
enum Backend {
    Local { directory: PathBuf },
    S3(S3Backend),
}

#[derive(Clone)]
pub struct StorageService {
    backend: Backend,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hmac_sha256(key: &[u8], value: &str) -> Result<[u8; 32], String> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| "Could not initialize S3 request signer".to_string())?;
    mac.update(value.as_bytes());
    let bytes = mac.finalize().into_bytes();
    let mut result = [0_u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn object_url(backend: &S3Backend, key: &str) -> Result<Url, String> {
    if let Some(endpoint) = &backend.endpoint_url {
        let mut url = Url::parse(endpoint)
            .map_err(|_| "The S3-compatible endpoint URL is invalid".to_string())?;
        if url.scheme() != "https" && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")) {
            return Err("S3-compatible endpoints must use HTTPS outside localhost".to_string());
        }
        let base = url.path().trim_end_matches('/');
        url.set_path(&format!("{base}/{}/{}", backend.bucket, key));
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    } else {
        let host = if backend.bucket.contains('.') {
            format!("s3.{}.amazonaws.com", backend.region)
        } else {
            format!("{}.s3.{}.amazonaws.com", backend.bucket, backend.region)
        };
        let path = if backend.bucket.contains('.') {
            format!("/{}/{}", backend.bucket, key)
        } else {
            format!("/{key}")
        };
        Url::parse(&format!("https://{host}{path}"))
            .map_err(|_| "Could not construct the S3 object URL".to_string())
    }
}

fn signed_headers(
    backend: &S3Backend,
    method: &Method,
    url: &Url,
    payload_hash: &str,
    content_type: Option<&str>,
) -> Result<HeaderMap, String> {
    if payload_hash.len() != 64 || !payload_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("S3 request payload checksum is invalid".to_string());
    }

    let now = chrono::Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();
    let host_name = url.host_str().ok_or_else(|| "S3 endpoint host is missing".to_string())?;
    let host = match url.port() {
        Some(port) => format!("{host_name}:{port}"),
        None => host_name.to_string(),
    };
    let canonical_uri = if url.path().is_empty() { "/" } else { url.path() };
    let mut canonical_values = vec![
        ("host", host.as_str()),
        ("x-amz-content-sha256", payload_hash),
        ("x-amz-date", amz_date.as_str()),
    ];
    let content_type = content_type.map(str::trim).filter(|value| !value.is_empty());
    if let Some(value) = content_type {
        canonical_values.push(("content-type", value));
    }
    if let Some(token) = backend.session_token.as_deref() {
        canonical_values.push(("x-amz-security-token", token));
    }
    canonical_values.sort_by(|left, right| left.0.cmp(right.0));

    let canonical_headers = canonical_values
        .iter()
        .map(|(name, value)| format!("{name}:{}\n", value.trim()))
        .collect::<String>();
    let signed_header_names = canonical_values
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        method.as_str(),
        canonical_uri,
        canonical_headers,
        signed_header_names,
        payload_hash,
    );
    let credential_scope = format!("{date_stamp}/{}/s3/aws4_request", backend.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );

    let mut initial_key = b"AWS4".to_vec();
    initial_key.extend_from_slice(backend.secret_access_key.as_bytes());
    let date_key = hmac_sha256(&initial_key, &date_stamp)?;
    let region_key = hmac_sha256(&date_key, &backend.region)?;
    let service_key = hmac_sha256(&region_key, "s3")?;
    let signing_key = hmac_sha256(&service_key, "aws4_request")?;
    let signature = hex(&hmac_sha256(&signing_key, &string_to_sign)?);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_header_names}, Signature={signature}",
        backend.access_key_id
    );

    let mut headers = HeaderMap::new();
    headers.insert(HOST, HeaderValue::from_str(&host).map_err(|_| "S3 host header is invalid".to_string())?);
    headers.insert(
        HeaderName::from_static("x-amz-content-sha256"),
        HeaderValue::from_str(payload_hash).map_err(|_| "S3 checksum header is invalid".to_string())?,
    );
    headers.insert(
        HeaderName::from_static("x-amz-date"),
        HeaderValue::from_str(&amz_date).map_err(|_| "S3 date header is invalid".to_string())?,
    );
    if let Some(value) = content_type {
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(value).map_err(|_| "S3 content type is invalid".to_string())?,
        );
    }
    if let Some(token) = backend.session_token.as_deref() {
        headers.insert(
            HeaderName::from_static("x-amz-security-token"),
            HeaderValue::from_str(token).map_err(|_| "S3 session token header is invalid".to_string())?,
        );
    }
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&authorization).map_err(|_| "S3 authorization header is invalid".to_string())?,
    );
    Ok(headers)
}

async fn s3_error(response: reqwest::Response, operation: &str) -> String {
    let status = response.status();
    let details = response.text().await.unwrap_or_default();
    let summary = details.chars().take(180).collect::<String>();
    format!("S3 {operation} failed with HTTP {status}: {summary}")
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
                let required = |name: &str| -> Result<String, std::io::Error> {
                    env::var(name)
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("{name} is required when storage mode is s3"),
                        ))
                };
                let bucket = required("CAMPUSFORGE_S3_BUCKET")?;
                let region = env::var("CAMPUSFORGE_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
                let access_key_id = required("AWS_ACCESS_KEY_ID")?;
                let secret_access_key = required("AWS_SECRET_ACCESS_KEY")?;
                let session_token = env::var("AWS_SESSION_TOKEN").ok().filter(|value| !value.trim().is_empty());
                let endpoint_url = env::var("CAMPUSFORGE_S3_ENDPOINT_URL").ok().filter(|value| !value.trim().is_empty());
                let client = Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    // SigV4 credentials include x-amz-security-token; never forward them across redirects.
                    .redirect(Policy::none())
                    .user_agent("CampusForgeTeams/1.0")
                    .build()
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string()))?;
                tracing::info!(bucket = %bucket, region = %region, "S3-compatible private object storage configured");
                Ok(Self {
                    backend: Backend::S3(S3Backend {
                        client, bucket, region, access_key_id, secret_access_key,
                        session_token, endpoint_url,
                    })
                })
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

    pub async fn put_file(
        &self,
        key: &str,
        source: &Path,
        content_type: &str,
        checksum_sha256: &str,
        size_bytes: u64,
    ) -> Result<(), String> {
        Self::validate_key(key)?;
        match &self.backend {
            Backend::Local { directory } => {
                let destination = directory.join(key);
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|error| error.to_string())?;
                }
                if let Err(error) = tokio::fs::copy(source, &destination).await {
                    let _ = tokio::fs::remove_file(&destination).await;
                    return Err(error.to_string());
                }
                Ok(())
            }
            Backend::S3(backend) => {
                let file = tokio::fs::File::open(source).await.map_err(|error| error.to_string())?;
                let body = reqwest::Body::wrap_stream(ReaderStream::new(file));
                let method = Method::PUT;
                let url = object_url(backend, key)?;
                let headers = signed_headers(backend, &method, &url, checksum_sha256, Some(content_type))?;
                let content_length = HeaderValue::from_str(&size_bytes.to_string())
                    .map_err(|_| "S3 upload size is invalid".to_string())?;
                let response = backend.client.put(url)
                    .headers(headers)
                    .header(CONTENT_LENGTH, content_length)
                    .body(body)
                    .send().await
                    .map_err(|error| format!("S3 upload request failed: {error}"))?;
                if !response.status().is_success() {
                    return Err(s3_error(response, "upload").await);
                }
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
            Backend::S3(backend) => {
                let method = Method::GET;
                let url = object_url(backend, key)?;
                let payload_hash = sha256_hex(&[]);
                let headers = signed_headers(backend, &method, &url, &payload_hash, None)?;
                let response = backend.client.get(url)
                    .headers(headers)
                    .send().await
                    .map_err(|error| format!("S3 download request failed: {error}"))?;
                if !response.status().is_success() {
                    return Err(s3_error(response, "download").await);
                }
                response.bytes().await.map(|bytes| bytes.to_vec())
                    .map_err(|error| format!("Could not read S3 download response: {error}"))
            }
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), String> {
        Self::validate_key(key)?;
        match &self.backend {
            Backend::Local { directory } => {
                match tokio::fs::remove_file(directory.join(key)).await {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error.to_string()),
                }
            }
            Backend::S3(backend) => {
                let method = Method::DELETE;
                let url = object_url(backend, key)?;
                let payload_hash = sha256_hex(&[]);
                let headers = signed_headers(backend, &method, &url, &payload_hash, None)?;
                let response = backend.client.delete(url)
                    .headers(headers)
                    .send().await
                    .map_err(|error| format!("S3 delete request failed: {error}"))?;
                if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_FOUND {
                    return Ok(());
                }
                Err(s3_error(response, "delete").await)
            }
        }
    }
}
