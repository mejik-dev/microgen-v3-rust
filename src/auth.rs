use crate::error::{check_status, MicrogenError, Result};
use serde::Serialize;
use std::sync::{Arc, Mutex};

/// Authentication client – login, register, social auth, password management.
#[derive(Debug, Clone)]
pub struct AuthClient {
    client: reqwest::Client,
    base_url: String,
    pub(crate) token: Arc<Mutex<Option<String>>>,
}

impl AuthClient {
    pub(crate) fn new(
        client: reqwest::Client,
        base_url: String,
        token: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            client,
            base_url,
            token,
        }
    }

    // ── helpers ──────────────────────────────────────────

    fn auth_header(&self) -> Option<String> {
        self.token
            .lock()
            .unwrap()
            .clone()
            .map(|v| format!("Bearer {v}"))
    }

    fn set_token(&self, token: String) {
        *self.token.lock().unwrap() = Some(token);
    }

    fn clear_token(&self) {
        *self.token.lock().unwrap() = None;
    }

    /// Return the current bearer token, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal token mutex is poisoned (indicates a prior panic while holding the lock).
    #[must_use]
    pub fn token(&self) -> Option<String> {
        self.token.lock().unwrap().clone()
    }

    /// Persist a token (called externally e.g. after restoring from storage).
    pub fn save_token(&self, token: String) {
        self.set_token(token);
    }

    async fn post_json<B: Serialize + Sync + ?Sized, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let resp = check_status(resp).await?;
        Ok(resp.json().await?)
    }

    async fn post_auth_json<B: Serialize + Sync + ?Sized, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url).json(body);
        if let Some(h) = self.auth_header() {
            req = req.header(reqwest::header::AUTHORIZATION, &h);
        }
        let resp = req.send().await?;
        let resp = check_status(resp).await?;
        Ok(resp.json().await?)
    }

    async fn get_auth<R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<R> {
        let mut url_str = format!("{}{}", self.base_url, path);
        if let Some(q) = query {
            if !q.is_empty() {
                let qs = serde_qs::to_string(q)
                    .map_err(|e| MicrogenError::InvalidArgument(e.to_string()))?;
                url_str.push('?');
                url_str.push_str(&qs);
            }
        }
        let mut req = self.client.get(&url_str);
        if let Some(h) = self.auth_header() {
            req = req.header(reqwest::header::AUTHORIZATION, &h);
        }
        let resp = req.send().await?;
        let resp = check_status(resp).await?;
        Ok(resp.json().await?)
    }

    async fn patch_auth<B: Serialize + Sync + ?Sized, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.patch(&url).json(body);
        if let Some(h) = self.auth_header() {
            req = req.header(reqwest::header::AUTHORIZATION, &h);
        }
        let resp = req.send().await?;
        let resp = check_status(resp).await?;
        Ok(resp.json().await?)
    }

    // ── public API ───────────────────────────────────────

    /// Register a new user.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn register<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<crate::types::TokenResponse<T>> {
        let tr: crate::types::TokenResponse<T> = self.post_json("/auth/register", body).await?;
        self.set_token(tr.token.clone());
        Ok(tr)
    }

    /// Login with email + password.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn login<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<crate::types::TokenResponse<T>> {
        let tr: crate::types::TokenResponse<T> = self.post_json("/auth/login", body).await?;
        self.set_token(tr.token.clone());
        Ok(tr)
    }

    /// Get the current user profile.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn user<T: serde::de::DeserializeOwned>(
        &self,
        option: Option<&crate::types::GetUserOption>,
    ) -> Result<T> {
        let mut query = None;
        if let Some(opt) = option {
            if let Some(ref lookup) = opt.lookup {
                let mut m = serde_json::Map::new();
                m.insert("$lookup".into(), lookup.clone());
                query = Some(m);
            }
        }
        self.get_auth::<T>("/auth/user", query.as_ref()).await
    }

    /// Update the current user profile.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn update<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T> {
        self.patch_auth("/auth/user", body).await
    }

    /// Logout.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn logout<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<crate::types::TokenResponse<T>> {
        let tr: crate::types::TokenResponse<T> = self
            .post_auth_json("/auth/logout", &serde_json::json!({}))
            .await?;
        self.clear_token();
        Ok(tr)
    }

    /// Verify the current (or provided) token is still valid.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn verify_token<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<crate::types::TokenResponse<T>> {
        self.post_auth_json("/auth/verify-token", &serde_json::json!({}))
            .await
    }

    /// Change password.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn change_password(
        &self,
        body: &serde_json::Value,
    ) -> Result<crate::types::ChangePasswordResponse> {
        self.post_auth_json("/auth/change-password", body).await
    }

    /// Begin a Regol QR handshake.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn login_with_regol_qr(
        &self,
        body: &serde_json::Value,
    ) -> Result<crate::types::AuthRegolResponse> {
        self.post_json("/auth/login/regol/qr", body).await
    }

    /// Login with a Google identity token.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn login_with_google<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<crate::types::TokenResponse<T>> {
        let tr: crate::types::TokenResponse<T> = self.post_json("/auth/login/google", body).await?;
        self.set_token(tr.token.clone());
        Ok(tr)
    }

    /// Login with a Facebook access token.
    ///
    /// # Errors
    ///
    /// Returns [`MicrogenError::Api`] if the server returns a non-success status,
    /// [`MicrogenError::Request`] on network failures,
    /// [`MicrogenError::Serde`] on JSON parse errors.
    pub async fn login_with_facebook<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<crate::types::TokenResponse<T>> {
        let tr: crate::types::TokenResponse<T> =
            self.post_json("/auth/login/facebook", body).await?;
        self.set_token(tr.token.clone());
        Ok(tr)
    }
}
