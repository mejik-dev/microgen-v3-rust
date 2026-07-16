use crate::error::{check_status, Result};
use crate::types::Storage;
use std::sync::{Arc, Mutex};

/// File storage client.
#[derive(Debug, Clone)]
pub struct StorageClient {
    client: reqwest::Client,
    upload_url: String,
    token: Arc<Mutex<Option<String>>>,
}

impl StorageClient {
    pub(crate) fn new(
        client: reqwest::Client,
        upload_url: String,
        token: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            client,
            upload_url,
            token,
        }
    }

    /// Upload a file.
    ///
    /// - `data` – raw file bytes.
    /// - `file_name` – the file name (required when `data` is a raw blob).
    /// - `token` – optional bearer token override (takes precedence over stored token).
    ///
    /// # Panics
    ///
    /// Panics if the internal token mutex is poisoned (indicates a prior panic while holding the lock).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MicrogenError::Api`] if the server returns a non-success status,
    /// [`crate::error::MicrogenError::Request`] on network failures,
    /// [`crate::error::MicrogenError::Serde`] on JSON parse errors.
    pub async fn upload(
        &self,
        data: Vec<u8>,
        file_name: &str,
        token: Option<&str>,
    ) -> Result<Storage> {
        let url = format!("{}/upload", self.upload_url);

        let part = reqwest::multipart::Part::bytes(data).file_name(file_name.to_string());
        let form = reqwest::multipart::Form::new().part("file", part);

        let mut req = self.client.post(&url).multipart(form);

        if let Some(t) = token {
            req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {t}"));
        } else if let Some(t) = self.token.lock().unwrap().clone() {
            req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {t}"));
        }

        let resp = req.send().await?;
        let resp = check_status(resp).await?;
        Ok(resp.json().await?)
    }
}
