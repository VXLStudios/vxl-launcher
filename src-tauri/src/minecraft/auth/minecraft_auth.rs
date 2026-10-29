use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;

use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use base64::Engine;

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use log::error;
use log::info;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding};
use rand::rngs::OsRng;
use rand::Rng;
use reqwest::header::HeaderMap;
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Digest;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::{ProjectDirsExt, HTTP_CLIENT, LAUNCHER_DIRECTORY};
use crate::minecraft::auth::token_storage::{
    self, load_account_secrets, store_account_secrets, AccountSecrets,
};
use crate::utils::backup_utils::{self, safe_write_with_backup, BackupConfig};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Credentials {
    pub id: Uuid,
    pub username: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    pub active: bool,
}

/// Account metadata exposed to the frontend — never includes tokens.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub id: Uuid,
    pub username: String,
    pub minecraft_username: String,
    pub active: bool,
    pub expires_at: DateTime<Utc>,
}

fn publish_log_redactions(accounts: &[Credentials]) {
    let pairs: Vec<(String, String)> = accounts
        .iter()
        .map(|account| (account.username.clone(), account.id.to_string()))
        .collect();
    crate::utils::security_utils::set_known_accounts(&pairs);
}

impl From<&Credentials> for PublicAccount {
    fn from(credentials: &Credentials) -> Self {
        Self {
            id: credentials.id,
            username: credentials.username.clone(),
            minecraft_username: credentials.username.clone(),
            active: credentials.active,
            expires_at: credentials.expires,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinecraftAuthStep {
    GetDeviceToken,
    SisuAuthenticate,
    GetOAuthToken,
    RefreshOAuthToken,
    SisuAuthorize,
    XstsAuthorize,
    MinecraftToken,
    MinecraftEntitlements,
    MinecraftProfile,
}

#[derive(thiserror::Error, Debug)]
pub enum MinecraftAuthenticationError {
    #[error("Error reading public key during generation")]
    ReadingPublicKey,
    #[error("Failed to serialize private key to PEM: {0}")]
    PEMSerialize(#[from] p256::pkcs8::Error),
    #[error("Failed to serialize body to JSON during step {step:?}: {source}")]
    SerializeBody {
        step: MinecraftAuthStep,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "Failed to deserialize response to JSON during step {step:?}: {source}. Status Code: {status_code} Body: {raw}"
    )]
    DeserializeResponse {
        step: MinecraftAuthStep,
        raw: String,
        #[source]
        source: serde_json::Error,
        status_code: reqwest::StatusCode,
    },
    #[error("Request failed during step {step:?}: {source}")]
    Request {
        step: MinecraftAuthStep,
        #[source]
        source: reqwest::Error,
    },
    #[error("Error creating signed request buffer {step:?}: {source}")]
    ConstructingSignedRequest {
        step: MinecraftAuthStep,
        #[source]
        source: std::io::Error,
    },
    #[error("Error reading XBOX Session ID header")]
    NoSessionId,
    #[error("Error reading user hash")]
    NoUserHash,
    #[error("This Microsoft account does not have a Minecraft Java Edition license. You may only have Bedrock Edition.")]
    NoMinecraftLicense,
    #[error("{0}")]
    XboxError(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SaveDeviceToken {
    pub id: String,
    pub private_key: String,
    pub x: String,
    pub y: String,
    pub token: DeviceToken,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftLoginFlow {
    pub verifier: String,
    pub challenge: String,
    pub session_id: String,
    pub redirect_uri: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DirectOAuthFlow {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
    pub redirect_uri: String,
    pub authorize_url: String,
}

pub struct MinecraftAuthStore {
    accounts: Arc<RwLock<Vec<Credentials>>>,
    store_path: PathBuf,
    token: Arc<RwLock<Option<SaveDeviceToken>>>,
    backup_config: BackupConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedAccount {
    id: Uuid,
    username: String,
    expires: DateTime<Utc>,
    active: bool,
    #[serde(default, skip_serializing)]
    access_token: Option<String>,
    #[serde(default, skip_serializing)]
    refresh_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AccountStore {
    accounts: Vec<PersistedAccount>,
    #[serde(default, skip_serializing)]
    token: Option<SaveDeviceToken>,
}

impl MinecraftAuthStore {
    pub async fn new() -> Result<Self> {
        // Create accounts directory if it doesn't exist
        let accounts_path = LAUNCHER_DIRECTORY.root_dir();
        if !accounts_path.exists() {
            std::fs::create_dir_all(&accounts_path)?;
        }

        let store_path = accounts_path.join("accounts.json");
        let manager = Self {
            accounts: Arc::new(RwLock::new(Vec::new())),
            store_path: store_path,
            token: Arc::new(RwLock::new(None)),
            backup_config: BackupConfig::default(),
        };

        manager.load().await?;
        Ok(manager)
    }

    async fn migrate_legacy_account(
        persisted: &mut PersistedAccount,
    ) -> Result<(String, String)> {
        if let (Some(access_token), Some(refresh_token)) = (
            persisted.access_token.take(),
            persisted.refresh_token.take(),
        ) {
            store_account_secrets(
                persisted.id,
                &AccountSecrets {
                    access_token: access_token.clone(),
                    refresh_token: refresh_token.clone(),
                },
            )?;
            return Ok((access_token, refresh_token));
        }

        let secrets = load_account_secrets(persisted.id)?;
        Ok((secrets.access_token, secrets.refresh_token))
    }

    async fn migrate_legacy_device_token(token: &mut Option<SaveDeviceToken>) -> Result<()> {
        if let Some(device_token) = token.take() {
            let json = serde_json::to_string(&device_token)?;
            token_storage::store_device_token_json(&json)?;
        }
        Ok(())
    }

    async fn load_device_token_from_keychain(&self) -> Result<Option<SaveDeviceToken>> {
        if let Some(json) = token_storage::load_device_token_json()? {
            let device_token: SaveDeviceToken = serde_json::from_str(&json).map_err(|e| {
                AppError::AccountError(format!("Failed to parse stored device token: {}", e))
            })?;
            Ok(Some(device_token))
        } else {
            Ok(None)
        }
    }

    pub async fn load(&self) -> Result<()> {
        info!("[Storage] Starting load operation");

        if self.store_path.try_exists()? {
            info!(
                "[Storage] Account file exists at: {}",
                self.store_path.display()
            );
            let data = fs::read_to_string(&self.store_path).await?;

            let mut store: AccountStore = match serde_json::from_str(&data) {
                Ok(store) => store,
                Err(e) => {
                    error!(
                        "[Storage] Failed to deserialize account data: {}. Attempting backup restore.",
                        e
                    );
                    if backup_utils::restore_from_backup(&self.store_path, Some("accounts"))
                        .await
                        .is_ok()
                    {
                        let restored = fs::read_to_string(&self.store_path).await?;
                        serde_json::from_str(&restored).unwrap_or(AccountStore {
                            accounts: Vec::new(),
                            token: None,
                        })
                    } else {
                        AccountStore {
                            accounts: Vec::new(),
                            token: None,
                        }
                    }
                }
            };

            let mut credentials = Vec::new();
            let mut migrated = false;

            for mut persisted in store.accounts {
                let had_legacy_tokens = persisted.access_token.is_some();
                let (access_token, refresh_token) =
                    match Self::migrate_legacy_account(&mut persisted).await {
                        Ok(tokens) => tokens,
                        Err(e) => {
                            error!(
                                "[Storage] Skipping account {} — could not load secrets: {}",
                                persisted.id, e
                            );
                            continue;
                        }
                    };

                if had_legacy_tokens {
                    migrated = true;
                }

                credentials.push(Credentials {
                    id: persisted.id,
                    username: persisted.username,
                    access_token,
                    refresh_token,
                    expires: persisted.expires,
                    active: persisted.active,
                });
            }

            if store.token.is_some() {
                migrated = true;
            }
            Self::migrate_legacy_device_token(&mut store.token).await?;

            let device_token = if let Some(token) = store.token {
                Some(token)
            } else {
                self.load_device_token_from_keychain().await?
            };

            publish_log_redactions(&credentials);
            {
                let mut accounts = self.accounts.write().await;
                *accounts = credentials;
            }
            {
                let mut token_guard = self.token.write().await;
                *token_guard = device_token;
            }

            if migrated {
                info!("[Storage] Migrated legacy account tokens to OS keychain");
                self.save().await?;
            }
        } else {
            info!("[Storage] No account file found, starting with empty accounts");
        }

        info!("[Storage] Load operation completed successfully");
        Ok(())
    }

    async fn save(&self) -> Result<()> {
        info!("[Storage] Starting save operation");

        let accounts = self.accounts.read().await;
        let device_token = self.token.read().await;

        for account in accounts.iter() {
            store_account_secrets(
                account.id,
                &AccountSecrets {
                    access_token: account.access_token.clone(),
                    refresh_token: account.refresh_token.clone(),
                },
            )?;
        }

        if let Some(device_token) = device_token.as_ref() {
            let json = serde_json::to_string(device_token)?;
            token_storage::store_device_token_json(&json)?;
        }

        let store = AccountStore {
            accounts: accounts
                .iter()
                .map(|account| PersistedAccount {
                    id: account.id,
                    username: account.username.clone(),
                    expires: account.expires,
                    active: account.active,
                    access_token: None,
                    refresh_token: None,
                })
                .collect(),
            token: None,
        };

        let data = serde_json::to_string_pretty(&store)?;
        safe_write_with_backup(
            &self.store_path,
            data.as_bytes(),
            Some("accounts"),
            &self.backup_config,
        )
        .await?;

        info!("[Storage] Save operation completed successfully");
        Ok(())
    }

    async fn refresh_and_get_device_token(
        &self,
        current_date: DateTime<Utc>,
        force_generate: bool,
    ) -> Result<(DeviceTokenKey, DeviceToken, DateTime<Utc>, bool)> {
        info!("refresh_and_get_device_token");

        // Prefer reusing the existing key unless explicitly forced to generate a new one
        if !force_generate {
            // Read current saved device token/key once
            let saved = {
                let current_token = self.token.read().await;
                current_token.clone()
            };

            if let Some(saved_token) = saved {
                // Parse existing private key and construct the key material
                let private_key = SigningKey::from_pkcs8_pem(&saved_token.private_key)
                    .map_err(|err| MinecraftAuthenticationError::PEMSerialize(err))?;

                let key = DeviceTokenKey {
                    id: saved_token.id.clone(),
                    key: private_key,
                    x: saved_token.x.clone(),
                    y: saved_token.y.clone(),
                };

                // If the cached token is still valid, return it directly without a refresh call
                if saved_token.token.not_after > current_date {
                    return Ok((key, saved_token.token.clone(), current_date, false));
                }

                // Otherwise, request a fresh device token using the same key
                let res = device_token(&key, current_date).await?;

                // Update only the token in storage (keep the same key)
                {
                    let mut token_guard = self.token.write().await;
                    if let Some(stored) = token_guard.as_mut() {
                        stored.token = res.value.clone();
                    }
                }
                self.save().await?;

                // false indicates we reused the existing key
                return Ok((key, res.value, res.date, false));
            }
        }

        // No existing key or forced generation: create a new key and token
        let key = generate_key()?;
        let res = device_token(&key, current_date).await?;

        let new_token = SaveDeviceToken {
            id: key.id.clone(),
            private_key: key
                .key
                .to_pkcs8_pem(LineEnding::default())
                .map_err(|err| MinecraftAuthenticationError::PEMSerialize(err))?
                .to_string(),
            x: key.x.clone(),
            y: key.y.clone(),
            token: res.value.clone(),
        };

        {
            let mut token = self.token.write().await;
            *token = Some(new_token);
        }

        self.save().await?;
        // true indicates a new key was generated
        Ok((key, res.value, res.date, true))
    }

    /// Starts a direct OAuth2 flow (for Flatpak/localhost redirect)
    /// This uses the direct OAuth2 endpoint instead of SISU
    pub async fn login_begin_direct_oauth(&self, redirect_uri: &str) -> Result<DirectOAuthFlow> {
        info!("[Direct OAuth Flow] Starting direct OAuth2 login");
        
        // Generate OAuth verifier (for PKCE)
        let verifier = generate_oauth_challenge();
        let mut hasher = sha2::Sha256::new();
        hasher.update(&verifier);
        let result = hasher.finalize();
        let challenge = BASE64_URL_SAFE_NO_PAD.encode(result);
        
        // Generate state
        let state = generate_oauth_challenge();
        
        // Build authorization URL for login.live.com
        let mut authorize_url = url::Url::parse(DIRECT_OAUTH_AUTHORIZE_URL)
            .map_err(|e| AppError::Other(format!("Failed to parse authorize URL: {}", e)))?;
        
        authorize_url.query_pairs_mut()
            .append_pair("client_id", DIRECT_OAUTH_CLIENT_ID)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", "XboxLive.signin offline_access")
            .append_pair("state", &state)
            .append_pair("prompt", "select_account");
        
        info!("[Direct OAuth Flow] Generated authorization URL");
        
        Ok(DirectOAuthFlow {
            verifier,
            challenge,
            state,
            redirect_uri: redirect_uri.to_string(),
            authorize_url: authorize_url.to_string(),
        })
    }

    pub async fn login_begin(&self) -> Result<MinecraftLoginFlow> {
        info!("[Auth Flow] Starting login_begin process");
        info!("[Auth Flow] Initializing device token refresh");
        let (key, token, current_date, valid_date) =
            self.refresh_and_get_device_token(Utc::now(), false).await?;

        info!("[Auth Flow] Generating OAuth challenge");
        let verifier = generate_oauth_challenge();
        let mut hasher = sha2::Sha256::new();
        hasher.update(&verifier);
        let result = hasher.finalize();
        let challenge = BASE64_URL_SAFE_NO_PAD.encode(result);

        match sisu_authenticate(&token.token, &challenge, &key, current_date).await {
            Ok((session_id, redirect_uri)) => {
                info!("[Auth Flow] SISU authentication successful");
                info!("[Auth Flow] Session ID generated: {}", session_id);
                Ok(MinecraftLoginFlow {
                    verifier,
                    challenge,
                    session_id,
                    redirect_uri: redirect_uri.value.msa_oauth_redirect,
                })
            }
            Err(err) => {
                info!("[Auth Flow] SISU authentication failed: {:?}", err);
                if !valid_date {
                    info!("[Auth Flow] Retrying with new device token due to invalid date");
                    let (key, token, current_date, _) =
                        self.refresh_and_get_device_token(Utc::now(), false).await?;

                    info!("[Auth Flow] Regenerating OAuth challenge for retry");
                    let verifier = generate_oauth_challenge();
                    let mut hasher = sha2::Sha256::new();
                    hasher.update(&verifier);
                    let result = hasher.finalize();
                    let challenge = BASE64_URL_SAFE_NO_PAD.encode(result);

                    info!("[Auth Flow] Retrying SISU authentication");
                    let (session_id, redirect_uri) =
                        sisu_authenticate(&token.token, &challenge, &key, current_date).await?;

                    info!(
                        "[Auth Flow] Retry successful - New session ID: {}",
                        session_id
                    );
                    Ok(MinecraftLoginFlow {
                        verifier,
                        challenge,
                        session_id,
                        redirect_uri: redirect_uri.value.msa_oauth_redirect,
                    })
                } else {
                    info!("[Auth Flow] Authentication failed and no retry possible");
                    Err(err)
                }
            }
        }
    }

    pub async fn login_finish(&self, code: &str, flow: MinecraftLoginFlow) -> Result<Credentials> {
        info!("[Auth Flow] Starting login_finish process");
        info!("[Auth Flow] Refreshing device token");
        let (key, token, _, _) = self.refresh_and_get_device_token(Utc::now(), false).await?;

        info!("[Auth Flow] Getting OAuth token");
        let oauth_token = oauth_token(code, &flow.verifier).await?;

        info!("[Auth Flow] Authorizing with SISU");
        let sisu_authorize = sisu_authorize(
            Some(&flow.session_id),
            &oauth_token.value.access_token,
            &token.token,
            &key,
            oauth_token.date,
        )
        .await?;

        info!("[Auth Flow] Authorizing with XSTS");
        let xbox_token = xsts_authorize(
            sisu_authorize.value,
            &token.token,
            &key,
            sisu_authorize.date,
        )
        .await?;

        info!("[Auth Flow] Getting Minecraft token");
        let minecraft_token = minecraft_token(xbox_token.value).await?;

        info!("[Auth Flow] Checking Minecraft entitlements");
        minecraft_entitlements(&minecraft_token.access_token).await?;

        info!("[Auth Flow] Fetching Minecraft profile");
        let profile = minecraft_profile(&minecraft_token.access_token).await?;
        info!(
            "[Auth Flow] Profile retrieved - ID: {:?}, Name: {}",
            profile.id, profile.name
        );

        let profile_id = profile.id.unwrap_or_default();
        info!("[Auth Flow] Using profile ID: {}", profile_id);

        let existing_account = self.get_account_by_id(profile_id).await?;
        info!(
            "[Auth Flow] Existing account found: {}",
            existing_account.is_some()
        );

        let credentials = Credentials {
            id: profile_id,
            active: true,
            username: profile.name,
            access_token: minecraft_token.access_token,
            refresh_token: oauth_token.value.refresh_token,
            expires: oauth_token.date + Duration::seconds(oauth_token.value.expires_in as i64),
        };

        info!(
            "[Auth Flow] Updating/inserting credentials for account: {}",
            credentials.username
        );
        self.update_or_insert(credentials.clone()).await?;
        info!("[Auth Flow] Login process completed successfully");

        Ok(credentials)
    }

    /// Completes the direct OAuth2 flow (for Flatpak/localhost redirect)
    pub async fn login_finish_direct_oauth(&self, code: &str, flow: DirectOAuthFlow) -> Result<Credentials> {
        info!("[Direct OAuth Flow] Starting login_finish_direct_oauth");
        
        // Exchange code for access token
        info!("[Direct OAuth Flow] Exchanging code for access token");
        let oauth_token = direct_oauth_token(code, &flow.verifier, &flow.redirect_uri).await?;
        
        // Exchange Microsoft access token for Xbox token (RPS method, no SISU)
        info!("[Direct OAuth Flow] Exchanging Microsoft token for Xbox token");
        let xbox_token = xbox_authenticate_rps(&oauth_token.value.access_token).await?;
        
        // Exchange Xbox token for XSTS token
        info!("[Direct OAuth Flow] Exchanging Xbox token for XSTS token");
        let xsts_token = xsts_authorize_direct(xbox_token).await?;
        
        // Get Minecraft token
        info!("[Direct OAuth Flow] Getting Minecraft token");
        let minecraft_token = minecraft_token(xsts_token).await?;
        
        // Check entitlements
        info!("[Direct OAuth Flow] Checking Minecraft entitlements");
        minecraft_entitlements(&minecraft_token.access_token).await?;
        
        // Get profile
        info!("[Direct OAuth Flow] Fetching Minecraft profile");
        let profile = minecraft_profile(&minecraft_token.access_token).await?;
        info!(
            "[Direct OAuth Flow] Profile retrieved - ID: {:?}, Name: {}",
            profile.id, profile.name
        );
        
        let profile_id = profile.id.unwrap_or_default();
        let existing_account = self.get_account_by_id(profile_id).await?;
        
        let credentials = Credentials {
            id: profile_id,
            active: true,
            username: profile.name,
            access_token: minecraft_token.access_token,
            refresh_token: oauth_token.value.refresh_token,
            expires: oauth_token.date + Duration::seconds(oauth_token.value.expires_in as i64),
        };

        self.update_or_insert(credentials.clone()).await?;
        info!("[Direct OAuth Flow] Login process completed successfully");
        
        Ok(credentials)
    }

    /// Completes the direct OAuth2 flow with event emission (for Flatpak/localhost redirect)
    pub async fn login_finish_direct_oauth_with_events(&self, code: &str, flow: DirectOAuthFlow, event_id: Uuid) -> Result<Credentials> {
        info!("[Direct OAuth Flow] Starting login_finish_direct_oauth");
        let state = crate::state::State::get().await?;
        
        // Exchange code for access token
        info!("[Direct OAuth Flow] Exchanging code for access token");
        let oauth_token = direct_oauth_token(code, &flow.verifier, &flow.redirect_uri).await
            .map_err(|e| {
                Self::emit_login_error_event(&state, event_id, format!("Failed to exchange authorization code: {}", e));
                e
            })?;
        
        // Exchange Microsoft access token for Xbox token (RPS method, no SISU)
        info!("[Direct OAuth Flow] Exchanging Microsoft token for Xbox token");
        Self::emit_login_progress_event(
            &state,
            event_id,
            crate::state::event_state::EventType::AccountLoginExchangingXboxToken,
            "Exchanging Microsoft token for Xbox token",
            Some(50.0),
        ).await?;
        let xbox_token = xbox_authenticate_rps(&oauth_token.value.access_token).await
            .map_err(|e| {
                Self::emit_login_error_event(&state, event_id, format!("Failed to authenticate with Xbox: {}", e));
                e
            })?;
        
        // Exchange Xbox token for XSTS token
        info!("[Direct OAuth Flow] Exchanging Xbox token for XSTS token");
        Self::emit_login_progress_event(
            &state,
            event_id,
            crate::state::event_state::EventType::AccountLoginExchangingXstsToken,
            "Exchanging Xbox token for XSTS token",
            Some(60.0),
        ).await?;
        let xsts_token = xsts_authorize_direct(xbox_token).await
            .map_err(|e| {
                Self::emit_login_error_event(&state, event_id, format!("Failed to authorize XSTS token: {}", e));
                e
            })?;
        
        // Get Minecraft token
        info!("[Direct OAuth Flow] Getting Minecraft token");
        Self::emit_login_progress_event(
            &state,
            event_id,
            crate::state::event_state::EventType::AccountLoginGettingMinecraftToken,
            "Getting Minecraft access token",
            Some(70.0),
        ).await?;
        let minecraft_token = minecraft_token(xsts_token).await
            .map_err(|e| {
                Self::emit_login_error_event(&state, event_id, format!("Failed to get Minecraft token: {}", e));
                e
            })?;
        
        // Check entitlements
        info!("[Direct OAuth Flow] Checking Minecraft entitlements");
        Self::emit_login_progress_event(
            &state,
            event_id,
            crate::state::event_state::EventType::AccountLoginCheckingEntitlements,
            "Checking Minecraft entitlements",
            Some(80.0),
        ).await?;
        minecraft_entitlements(&minecraft_token.access_token).await
            .map_err(|e| {
                Self::emit_login_error_event(&state, event_id, format!("Failed to check Minecraft entitlements: {}", e));
                e
            })?;
        
        // Get profile
        info!("[Direct OAuth Flow] Fetching Minecraft profile");
        Self::emit_login_progress_event(
            &state,
            event_id,
            crate::state::event_state::EventType::AccountLoginFetchingProfile,
            "Fetching Minecraft profile",
            Some(90.0),
        ).await?;
        let profile = minecraft_profile(&minecraft_token.access_token).await
            .map_err(|e| {
                Self::emit_login_error_event(&state, event_id, format!("Failed to fetch Minecraft profile: {}", e));
                e
            })?;
        info!(
            "[Direct OAuth Flow] Profile retrieved - ID: {:?}, Name: {}",
            profile.id, profile.name
        );
        
        let profile_id = profile.id.unwrap_or_default();
        let existing_account = self.get_account_by_id(profile_id).await?;
        
        let credentials = Credentials {
            id: profile_id,
            active: true,
            username: profile.name,
            access_token: minecraft_token.access_token,
            refresh_token: oauth_token.value.refresh_token,
            expires: oauth_token.date + Duration::seconds(oauth_token.value.expires_in as i64),
        };

        self.update_or_insert(credentials.clone()).await?;
        info!("[Direct OAuth Flow] Login process completed successfully");
        
        Ok(credentials)
    }

    async fn refresh_token(&self, creds: &Credentials) -> Result<Option<Credentials>> {
        info!(
            "[Token Refresh] Starting token refresh for account: {}",
            creds.username
        );
        let cred_id = creds.id;
        let profile_name = creds.username.clone();

        info!("[Token Refresh] Getting OAuth refresh token");
        let oauth_token = oauth_refresh(&creds.refresh_token).await?;

        info!("[Token Refresh] Refreshing device token");
        let (key, token, current_date, _) = self
            .refresh_and_get_device_token(oauth_token.date, false)
            .await?;

        info!("[Token Refresh] Authorizing with SISU");
        let sisu_authorize = sisu_authorize(
            None,
            &oauth_token.value.access_token,
            &token.token,
            &key,
            current_date,
        )
        .await?;

        info!("[Token Refresh] Authorizing with XSTS");
        let xbox_token = xsts_authorize(
            sisu_authorize.value,
            &token.token,
            &key,
            sisu_authorize.date,
        )
        .await?;

        info!("[Token Refresh] Getting Minecraft token");
        let minecraft_token = minecraft_token(xbox_token.value).await?;

        info!("[Token Refresh] Creating new credentials");
        let val = Credentials {
            id: cred_id,
            username: profile_name,
            access_token: minecraft_token.access_token,
            refresh_token: oauth_token.value.refresh_token,
            expires: oauth_token.date + Duration::seconds(oauth_token.value.expires_in as i64),
            active: creds.clone().active,
        };

        info!("[Token Refresh] Updating account in storage");
        self.update_or_insert(val.clone()).await?;
        info!("[Token Refresh] Token refresh completed successfully");

        Ok(Some(val))
    }

    pub async fn get_account_by_id(&self, id: Uuid) -> Result<Option<Credentials>> {
        let accounts = self.accounts.read().await;
        Ok(accounts.iter().find(|acc| acc.id == id).cloned())
    }

    /// Gets an account by ID and refreshes its tokens if necessary.
    /// 
    /// This method retrieves an account by its ID, refreshes Microsoft and NoRisk tokens
    /// if needed, updates the account in storage, and returns the refreshed credentials.
    /// 
    /// # Arguments
    /// * `id` - The UUID of the account to retrieve
    /// * `experimental_mode` - Whether to use experimental mode for NoRisk token refresh
    /// 
    /// # Returns
    /// * `Ok(Some(Credentials))` - The refreshed account credentials
    /// * `Ok(None)` - Account not found
    /// * `Err` - Error during retrieval or refresh
    pub async fn get_account_by_id_with_refresh(
        &self,
        id: Uuid,
        experimental_mode: bool,
    ) -> Result<Option<Credentials>> {
        info!(
            "[Account Manager] Getting account by ID with refresh: {}",
            id
        );

        // Get the account from storage
        let account = self.get_account_by_id(id).await?;

        if let Some(creds) = account {
            info!(
                "[Account Manager] Found account: {}. Refreshing tokens.",
                creds.username
            );

            // Refresh tokens if needed
            let updated_account = self
                .update_microsoft_token(&creds)
                .await?;

            if let Some(updated) = updated_account {
                // Update account in storage after refresh
                {
                    info!("[Account Manager] Acquiring write lock to update account");
                    let mut accounts = self.accounts.write().await;
                    info!("[Account Manager] Successfully acquired write lock");
                    if let Some(existing) = accounts.iter_mut().find(|acc| acc.id == updated.id) {
                        info!("[Account Manager] Updating account in list");
                        *existing = updated.clone();
                    }
                    info!("[Account Manager] Releasing write lock");
                } // Write-Lock wird hier freigegeben

                info!("[Account Manager] Saving updated account");
                self.save().await?;
                info!("[Account Manager] Successfully saved account");

                Ok(Some(updated))
            } else {
                info!("[Account Manager] Token refresh returned None, using original credentials");
                Ok(Some(creds))
            }
        } else {
            info!("[Account Manager] Account with ID {} not found", id);
            Ok(None)
        }
    }

    pub async fn update_or_insert(&self, credentials: Credentials) -> Result<()> {
        info!("[Account Manager] Starting account update/insert operation");
        info!("[Account Manager] Account ID: {}", credentials.id);
        info!("[Account Manager] Username: {}", credentials.username);

        {
            let mut accounts = self.accounts.write().await;

            // Wenn der Account existiert, aktualisiere ihn
            if let Some(existing) = accounts.iter_mut().find(|acc| acc.id == credentials.id) {
                info!("[Account Manager] Found existing account, updating credentials");
                *existing = credentials;
                info!("[Account Manager] Account successfully updated");
            } else {
                // Wenn der Account nicht existiert, füge ihn hinzu
                info!("[Account Manager] No existing account found, creating new account");
                accounts.push(credentials);
                info!("[Account Manager] New account successfully created");
            }
        } // Write-Lock wird hier automatisch freigegeben

        info!("[Account Manager] Saving account changes to storage");
        self.save().await?;
        info!("[Account Manager] Account changes successfully saved");

        Ok(())
    }

    pub async fn update_microsoft_token(
        &self,
        creds: &Credentials,
    ) -> Result<Option<Credentials>> {
        info!(
            "[Token Check] Starting Microsoft token validation check for user: {}",
            creds.username
        );
        info!(
            "[Token Check] Microsoft token expires at: {}",
            creds.expires
        );

        if creds.expires <= Utc::now() + Duration::minutes(5) {
            info!("[Token Check] Microsoft token nearing expiry, initiating proactive refresh");
            let old_credentials = creds.clone();

            let res = self.refresh_token(&old_credentials).await;

            match res {
                Ok(val) => {
                    return if val.is_some() {
                        info!("[Token Check] Successfully refreshed Microsoft token");
                        Ok(val)
                    } else {
                        info!("[Token Check] Failed to refresh Microsoft token - No credentials found");
                        Err(AppError::NoCredentialsError)
                    };
                }
                Err(err) => {
                    if let AppError::MinecraftAuthenticationError(
                        MinecraftAuthenticationError::Request { ref source, .. },
                    ) = err
                    {
                        if source.is_connect() || source.is_timeout() {
                            info!("[Token Check] Connection error during refresh, using old credentials");
                            return Ok(Some(old_credentials));
                        }
                    }
                    info!("[Token Check] Error during token refresh: {:?}", err);
                    Err(err)
                }
            }
        } else {
            info!("[Token Check] Microsoft token is still valid");
            Ok(Some(creds.clone()))
        }
    }

    pub async fn get_active_account(&self) -> Result<Option<Credentials>> {
        info!("[Account Manager] Starting get_active_account process");

        // Get the global state to check the experimental mode
        let state = crate::state::State::get().await?;
        let is_experimental = state.config_manager.is_experimental_mode().await;
        info!(
            "[Account Manager] Global experimental mode is: {}",
            is_experimental
        );

        // Zuerst nur lesen um den aktiven Account zu finden
        let active_account = {
            info!("[Account Manager] Acquiring read lock to find active account");
            let accounts = self.accounts.read().await;
            info!("[Account Manager] Successfully acquired read lock");
            let account = accounts.iter().find(|acc| acc.active).cloned();
            info!(
                "[Account Manager] Active account found: {}",
                account.is_some()
            );
            account
        };

        if let Some(account) = active_account {
            info!(
                "[Account Manager] Refreshing credentials for active account: {}",
                account.username
            );
            // Refresh credentials if needed
            let updated_account = self
                .update_microsoft_token(&account)
                .await?;

            if let Some(updated) = updated_account {
                // Aktualisiere den Account in der Liste
                {
                    info!("[Account Manager] Acquiring write lock to update account");
                    let mut accounts = self.accounts.write().await;
                    info!("[Account Manager] Successfully acquired write lock");
                    if let Some(existing) = accounts.iter_mut().find(|acc| acc.id == updated.id) {
                        info!("[Account Manager] Updating account in list");
                        *existing = updated.clone();
                    }
                    info!("[Account Manager] Releasing write lock");
                } // Write-Lock wird hier freigegeben

                info!("[Account Manager] Saving updated account");
                self.save().await?;
                info!("[Account Manager] Successfully saved account");

                Ok(Some(updated))
            } else {
                Ok(Some(account))
            }
        } else {
            info!("[Account Manager] No active account found, checking for any accounts");

            // Wenn kein Account aktiv ist, aber Accounts existieren, setze den ersten als aktiv
            let first_account = {
                let mut accounts = self.accounts.write().await;
                if let Some(first_account) = accounts.first_mut() {
                    info!(
                        "[Account Manager] Setting first account as active: {}",
                        first_account.username
                    );
                    first_account.active = true;
                    Some(first_account.clone())
                } else {
                    None
                }
            }; // Write-Lock wird hier freigegeben

            if let Some(account) = first_account {
                info!("[Account Manager] Saving changes");
                self.save().await?;
                info!("[Account Manager] Successfully saved changes");
                Ok(Some(account))
            } else {
                info!("[Account Manager] No accounts found at all");
                Ok(None)
            }
        }
    }

    pub async fn remove_account(&self, id: Uuid) -> Result<()> {
        info!("[Account Manager] Starting account removal for ID: {}", id);

        {
            info!("[Account Manager] Acquiring write lock for account removal");
            let mut accounts = self.accounts.write().await;
            info!("[Account Manager] Successfully acquired write lock");

            let initial_count = accounts.len();
            accounts.retain(|acc| acc.id != id);
            let final_count = accounts.len();

            if initial_count == final_count {
                info!("[Account Manager] Warning: No account found with ID {}", id);
            } else {
                info!("[Account Manager] Successfully removed account");
            }
            info!("[Account Manager] Releasing write lock");
        } // Write-Lock wird hier freigegeben

        info!("[Account Manager] Saving changes after account removal");
        token_storage::delete_account_secrets(id)?;
        self.save().await?;
        info!("[Account Manager] Successfully saved changes");

        Ok(())
    }

    pub async fn get_all_public_accounts(&self) -> Result<Vec<PublicAccount>> {
        let accounts = self.accounts.read().await;
        Ok(accounts.iter().map(PublicAccount::from).collect())
    }

    pub async fn get_all_accounts(&self) -> Result<Vec<Credentials>> {
        info!("[Account Manager] Starting get_all_accounts operation");

        info!("[Account Manager] Acquiring read lock");
        let accounts = self.accounts.read().await;
        info!("[Account Manager] Successfully acquired read lock");

        info!("[Account Manager] Found {} accounts", accounts.len());
        let accounts_clone = accounts.clone();

        info!("[Account Manager] Returning all accounts");
        Ok(accounts_clone)
    }

    pub async fn set_active_account(&self, account_id: Uuid) -> Result<()> {
        info!("[Account Manager] Starting set_active_account operation");
        info!("[Account Manager] Setting account {} as active", account_id);

        {
            info!("[Account Manager] Acquiring write lock");
            let mut accounts = self.accounts.write().await;
            info!("[Account Manager] Successfully acquired write lock");

            // Set all accounts to inactive first
            info!("[Account Manager] Deactivating all accounts");
            for account in accounts.iter_mut() {
                account.active = false;
            }

            // Find and set the specified account as active
            if let Some(account) = accounts.iter_mut().find(|acc| acc.id == account_id) {
                info!("[Account Manager] Found account, setting as active");
                account.active = true;
            } else {
                info!("[Account Manager] Warning: Account not found");
                return Err(AppError::AccountError(format!(
                    "Account with ID {} not found",
                    account_id
                )));
            }

            info!("[Account Manager] Releasing write lock");
        } // Write-Lock wird hier freigegeben

        info!("[Account Manager] Saving changes");
        self.save().await?;
        info!("[Account Manager] Successfully saved changes");

        Ok(())
    }

    /// Helper method to emit login progress events
    pub async fn emit_login_progress_event(
        state: &Arc<crate::state::State>,
        event_id: Uuid,
        event_type: crate::state::event_state::EventType,
        message: &str,
        progress: Option<f64>,
    ) -> Result<()> {
        state.emit_event(crate::state::event_state::EventPayload {
            event_id,
            event_type,
            target_id: None,
            message: message.to_string(),
            progress,
            error: None,
        }).await
    }

    /// Helper method to emit login error events
    pub fn emit_login_error_event(
        state: &Arc<crate::state::State>,
        event_id: Uuid,
        error_message: String,
    ) {
        let state_clone = Arc::clone(state);
        tokio::spawn(async move {
            let _ = state_clone.emit_event(crate::state::event_state::EventPayload {
                event_id,
                event_type: crate::state::event_state::EventType::Error,
                target_id: None,
                message: error_message.clone(),
                progress: None,
                error: Some(error_message),
            }).await;
        });
    }
}

const MICROSOFT_CLIENT_ID: &str = "00000000402b5328";
const AUTH_REPLY_URL: &str = "https://login.live.com/oauth20_desktop.srf";
const REQUESTED_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

// Alternative Client-ID for direct OAuth2 flow (supports localhost redirect)
const DIRECT_OAUTH_CLIENT_ID: &str = "e16699bb-2aa8-46da-b5e3-45cbcce29091";
const DIRECT_OAUTH_AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const DIRECT_OAUTH_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

pub struct RequestWithDate<T> {
    pub date: DateTime<Utc>,
    pub value: T,
}

// flow steps
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceToken {
    pub issue_instant: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub token: String,
    pub display_claims: HashMap<String, serde_json::Value>,
}

pub async fn device_token(
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>> {
    let res = send_signed_request(
        None,
        "https://device.auth.xboxlive.com/device/authenticate",
        "/device/authenticate",
        json!({
            "Properties": {
                "AuthMethod": "ProofOfPossession",
                "Id": format!("{{{}}}", key.id.to_uppercase()),
                "DeviceType": "Win32",
                "Version": "10.16.0",
                "ProofKey": {
                    "kty": "EC",
                    "x": key.x,
                    "y": key.y,
                    "crv": "P-256",
                    "alg": "ES256",
                    "use": "sig"
                }
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"

        }),
        key,
        MinecraftAuthStep::GetDeviceToken,
        current_date,
    )
    .await?;

    Ok(RequestWithDate {
        date: res.current_date,
        value: res.body,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RedirectUri {
    pub msa_oauth_redirect: String,
}

async fn sisu_authenticate(
    token: &str,
    challenge: &str,
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<(String, RequestWithDate<RedirectUri>)> {
    let res = send_signed_request::<RedirectUri>(
        None,
        "https://sisu.xboxlive.com/authenticate",
        "/authenticate",
        json!({
          "AppId": MICROSOFT_CLIENT_ID,
          "DeviceToken": token,
          "Offers": [
            REQUESTED_SCOPE
          ],
          "Query": {
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "state": generate_oauth_challenge(),
            "prompt": "select_account"
          },
          "RedirectUri": AUTH_REPLY_URL,
          "Sandbox": "RETAIL",
          "TokenType": "code",
          "TitleId": "1794566092",
        }),
        key,
        MinecraftAuthStep::SisuAuthenticate,
        current_date,
    )
    .await?;

    let session_id = res
        .headers
        .get("X-SessionId")
        .and_then(|x| x.to_str().ok())
        .ok_or_else(|| MinecraftAuthenticationError::NoSessionId)?
        .to_string();

    Ok((
        session_id,
        RequestWithDate {
            date: res.current_date,
            value: res.body,
        },
    ))
}

#[derive(Deserialize)]
struct OAuthToken {
    // pub token_type: String,
    pub expires_in: u64,
    // pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
    // pub user_id: String,
    // pub foci: String,
}

async fn oauth_token(code: &str, verifier: &str) -> Result<RequestWithDate<OAuthToken>> {
    let mut query = HashMap::new();
    query.insert("client_id", "00000000402b5328");
    query.insert("code", code);
    query.insert("code_verifier", verifier);
    query.insert("grant_type", "authorization_code");
    query.insert("redirect_uri", AUTH_REPLY_URL);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        HTTP_CLIENT
            .post("https://login.live.com/oauth20_token.srf")
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

async fn oauth_refresh(refresh_token: &str) -> Result<RequestWithDate<OAuthToken>> {
    let mut query = HashMap::new();
    query.insert("client_id", "00000000402b5328");
    query.insert("refresh_token", refresh_token);
    query.insert("grant_type", "refresh_token");
    query.insert("redirect_uri", AUTH_REPLY_URL);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        HTTP_CLIENT
            .post("https://login.live.com/oauth20_token.srf")
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::RefreshOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::RefreshOAuthToken,
        })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::RefreshOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

/// Direct OAuth2 token exchange (for localhost redirect)
async fn direct_oauth_token(
    code: &str,
    _verifier: &str,
    redirect_uri: &str,
) -> Result<RequestWithDate<OAuthToken>> {
    let mut query = HashMap::new();
    query.insert("client_id", DIRECT_OAUTH_CLIENT_ID);
    query.insert("code", code);
    query.insert("grant_type", "authorization_code");
    query.insert("redirect_uri", redirect_uri);
    query.insert("scope", "XboxLive.signin offline_access");

    let res = auth_retry(|| {
        HTTP_CLIENT
            .post(DIRECT_OAUTH_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

/// Xbox authentication using RPS method (direct, no SISU)
async fn xbox_authenticate_rps(access_token: &str) -> Result<String> {
    let res = auth_retry(|| {
        HTTP_CLIENT
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={}", access_token)
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::SisuAuthorize,
    })?;

    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::SisuAuthorize,
        })?;

    #[derive(Deserialize)]
    struct XboxResponse {
        #[serde(rename = "Token")]
        token: String,
    }

    let body: XboxResponse = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::SisuAuthorize,
            status_code: status,
        }
    })?;

    Ok(body.token)
}

/// Xbox error response structure for XSTS authorization failures
#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct XboxErrorResponse {
    XErr: Option<u64>,
    Message: Option<String>,
    Redirect: Option<String>,
}

/// Converts Xbox error codes to user-friendly messages
fn xbox_error_to_message(xerr: u64, redirect: Option<&str>) -> String {
    match xerr {
        2148916233 => "This Microsoft account doesn't have an Xbox account. Please create one at xbox.com or sign in to the Xbox app first.".to_string(),
        2148916235 => "This is a child account that needs parental approval. Please have a parent approve Xbox Live access.".to_string(),
        2148916236 | 2148916237 => "Xbox Live is not available in your country/region or adult verification is required.".to_string(),
        2148916238 => "This is a child account. Child accounts cannot access Minecraft without parental setup.".to_string(),
        2148916222 => "Xbox authentication failed. Please ensure your Microsoft account is properly set up with Xbox Live and try again. If the problem persists, try signing in to the Xbox app first.".to_string(),
        _ => format!(
            "Xbox authentication failed (Error code: {}). {}",
            xerr,
            redirect.map(|r| format!("Please visit: {}", r)).unwrap_or_default()
        ),
    }
}

/// XSTS authorization for direct OAuth flow
async fn xsts_authorize_direct(xbox_token: String) -> Result<DeviceToken> {
    let res = auth_retry(|| {
        HTTP_CLIENT
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&json!({
                "Properties": {
                    "SandboxId": "RETAIL",
                    "UserTokens": [xbox_token]
                },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT"
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::XstsAuthorize,
    })?;

    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::XstsAuthorize,
        })?;

    // Check for Xbox error response (401 Unauthorized with XErr code)
    if status == reqwest::StatusCode::UNAUTHORIZED {
        if let Ok(xbox_error) = serde_json::from_str::<XboxErrorResponse>(&text) {
            if let Some(xerr) = xbox_error.XErr {
                let message = xbox_error_to_message(xerr, xbox_error.Redirect.as_deref());
                info!("Xbox authentication error: XErr={}, Message={:?}", xerr, xbox_error.Message);
                return Err(MinecraftAuthenticationError::XboxError(message).into());
            }
        }
    }

    let body: DeviceToken = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::XstsAuthorize,
            status_code: status,
        }
    })?;

    Ok(body)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct SisuAuthorize {
    // pub authorization_token: DeviceToken,
    // pub device_token: String,
    // pub sandbox: String,
    pub title_token: DeviceToken,
    pub user_token: DeviceToken,
    // pub web_page: String,
}

async fn sisu_authorize(
    session_id: Option<&str>,
    access_token: &str,
    device_token: &str,
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<RequestWithDate<SisuAuthorize>> {
    let res = send_signed_request(
        None,
        "https://sisu.xboxlive.com/authorize",
        "/authorize",
        json!({
            "AccessToken": format!("t={access_token}"),
            "AppId": "00000000402b5328",
            "DeviceToken": device_token,
            "ProofKey": {
                "kty": "EC",
                "x": key.x,
                "y": key.y,
                "crv": "P-256",
                "alg": "ES256",
                "use": "sig"
            },
            "Sandbox": "RETAIL",
            "SessionId": session_id,
            "SiteName": "user.auth.xboxlive.com",
            "RelyingParty": "http://xboxlive.com",
            "UseModernGamertag": true
        }),
        key,
        MinecraftAuthStep::SisuAuthorize,
        current_date,
    )
    .await?;

    Ok(RequestWithDate {
        date: res.current_date,
        value: res.body,
    })
}

async fn xsts_authorize(
    authorize: SisuAuthorize,
    device_token: &str,
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>> {
    let res = send_signed_request(
        None,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        "/xsts/authorize",
        json!({
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [authorize.user_token.token],
                "DeviceToken": device_token,
                "TitleToken": authorize.title_token.token,
            },
        }),
        key,
        MinecraftAuthStep::XstsAuthorize,
        current_date,
    )
    .await?;

    Ok(RequestWithDate {
        date: res.current_date,
        value: res.body,
    })
}

#[derive(Deserialize)]
struct MinecraftToken {
    // pub username: String,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u64,
}

async fn minecraft_token(
    token: DeviceToken,
) -> std::result::Result<MinecraftToken, MinecraftAuthenticationError> {
    let uhs = token
        .display_claims
        .get("xui")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("uhs"))
        .and_then(|x| x.as_str().map(String::from))
        .ok_or_else(|| MinecraftAuthenticationError::NoUserHash)?;

    let token = token.token;

    let res = auth_retry(|| {
        HTTP_CLIENT
            .post("https://api.minecraftservices.com/launcher/login")
            .header("Accept", "application/json")
            .json(&json!({
                "platform": "PC_LAUNCHER",
                "xtoken": format!("XBL3.0 x={uhs};{token}"),
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftToken,
    })?;

    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftToken,
        })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::MinecraftToken,
            status_code: status,
        }
    })
}

#[derive(Deserialize, Debug)]
struct MinecraftProfile {
    pub id: Option<Uuid>,
    pub name: String,
}

async fn minecraft_profile(
    token: &str,
) -> std::result::Result<MinecraftProfile, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        HTTP_CLIENT
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Accept", "application/json")
            .bearer_auth(token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftProfile,
    })?;

    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftProfile,
        })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::MinecraftProfile,
            status_code: status,
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {}

async fn minecraft_entitlements(
    token: &str,
) -> std::result::Result<MinecraftEntitlements, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        HTTP_CLIENT
            .get(format!(
                "https://api.minecraftservices.com/entitlements/license?requestId={}",
                Uuid::new_v4()
            ))
            .header("Accept", "application/json")
            .bearer_auth(token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftEntitlements,
    })?;

    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftEntitlements,
        })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::MinecraftEntitlements,
            status_code: status,
        }
    })
}

// auth utils
async fn auth_retry<F>(
    reqwest_request: impl Fn() -> F,
) -> std::result::Result<reqwest::Response, reqwest::Error>
where
    F: Future<Output = std::result::Result<Response, reqwest::Error>>,
{
    const RETRY_COUNT: usize = 5; // Does command 9 times
    const RETRY_WAIT: std::time::Duration = std::time::Duration::from_millis(250);

    let mut resp = reqwest_request().await;
    for i in 0..RETRY_COUNT {
        match &resp {
            Ok(_) => {
                break;
            }
            Err(err) => {
                if err.is_connect() || err.is_timeout() {
                    if i < RETRY_COUNT - 1 {
                        info!("Request failed with connect error, retrying...",);
                        tokio::time::sleep(RETRY_WAIT).await;
                        resp = reqwest_request().await;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    resp
}

pub struct DeviceTokenKey {
    pub id: String,
    pub key: SigningKey,
    pub x: String,
    pub y: String,
}

fn generate_key() -> Result<DeviceTokenKey> {
    let id = Uuid::new_v4().to_string().to_uppercase();

    let signing_key = SigningKey::random(&mut OsRng);
    let public_key = VerifyingKey::from(&signing_key);

    let encoded_point = public_key.to_encoded_point(false);

    Ok(DeviceTokenKey {
        id,
        key: signing_key,
        x: BASE64_URL_SAFE_NO_PAD.encode(
            encoded_point
                .x()
                .ok_or_else(|| MinecraftAuthenticationError::ReadingPublicKey)?,
        ),
        y: BASE64_URL_SAFE_NO_PAD.encode(
            encoded_point
                .y()
                .ok_or_else(|| MinecraftAuthenticationError::ReadingPublicKey)?,
        ),
    })
}

struct SignedRequestResponse<T> {
    pub headers: HeaderMap,
    pub current_date: DateTime<Utc>,
    pub body: T,
}

async fn send_signed_request<T: DeserializeOwned>(
    authorization: Option<&str>,
    url: &str,
    url_path: &str,
    raw_body: serde_json::Value,
    key: &DeviceTokenKey,
    step: MinecraftAuthStep,
    current_date: DateTime<Utc>,
) -> Result<SignedRequestResponse<T>> {
    let auth = authorization.map_or(Vec::new(), |v| v.as_bytes().to_vec());

    let body = serde_json::to_vec(&raw_body)
        .map_err(|source| MinecraftAuthenticationError::SerializeBody { source, step })?;
    let time: u128 = { ((current_date.timestamp() as u128) + 11644473600) * 10000000 };

    let mut buffer = Vec::new();
    buffer.extend_from_slice(&1_u32.to_be_bytes()[..]);
    buffer.push(0_u8);
    buffer.extend_from_slice(&(time as u64).to_be_bytes()[..]);
    buffer.push(0_u8);
    buffer.extend_from_slice("POST".as_bytes());
    buffer.push(0_u8);
    buffer.extend_from_slice(url_path.as_bytes());
    buffer.push(0_u8);
    buffer.extend_from_slice(&auth);
    buffer.push(0_u8);
    buffer.extend_from_slice(&body);
    buffer.push(0_u8);

    let ecdsa_sig: Signature = key.key.sign(&buffer);

    let mut sig_buffer = Vec::new();
    sig_buffer.extend_from_slice(&1_i32.to_be_bytes()[..]);
    sig_buffer.extend_from_slice(&(time as u64).to_be_bytes()[..]);
    sig_buffer.extend_from_slice(&ecdsa_sig.r().to_bytes());
    sig_buffer.extend_from_slice(&ecdsa_sig.s().to_bytes());

    let signature = BASE64_STANDARD.encode(&sig_buffer);

    let res = auth_retry(|| {
        let mut request = HTTP_CLIENT
            .post(url)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Accept", "application/json")
            .header("Signature", &signature);

        if url != "https://sisu.xboxlive.com/authorize" {
            request = request.header("x-xbl-contract-version", "1");
        }

        if let Some(auth) = authorization {
            request = request.header("Authorization", auth);
        }

        request.body(body.clone()).send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;

    let status = res.status();
    let headers = res.headers().clone();

    let current_date = get_date_header(&headers);

    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;

    // Handle empty response body from Microsoft (status 400 with empty body)
    if text.trim().is_empty() {
        return Err(AppError::MinecraftAuthenticationError(MinecraftAuthenticationError::DeserializeResponse {
            source: serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Empty response body")),
            raw: text,
            step,
            status_code: status,
        }));
    }

    // Check for Xbox error response (401 Unauthorized with XErr code) in XSTS authorization
    if status == reqwest::StatusCode::UNAUTHORIZED && step == MinecraftAuthStep::XstsAuthorize {
        if let Ok(xbox_error) = serde_json::from_str::<XboxErrorResponse>(&text) {
            if let Some(xerr) = xbox_error.XErr {
                let message = xbox_error_to_message(xerr, xbox_error.Redirect.as_deref());
                info!("Xbox authentication error: XErr={}, Message={:?}", xerr, xbox_error.Message);
                return Err(MinecraftAuthenticationError::XboxError(message).into());
            }
        }
    }

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step,
            status_code: status,
        }
    })?;
    Ok(SignedRequestResponse {
        headers,
        current_date,
        body,
    })
}

fn get_date_header(headers: &HeaderMap) -> DateTime<Utc> {
    headers
        .get(reqwest::header::DATE)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| DateTime::parse_from_rfc2822(x).ok())
        .map_or(Utc::now(), |x| x.with_timezone(&Utc))
}

fn generate_oauth_challenge() -> String {
    let mut rng = rand::thread_rng();

    let bytes: Vec<u8> = (0..64).map(|_| rng.gen::<u8>()).collect();
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

/// Start an OAuth callback server that listens for the authorization code
pub async fn start_oauth_callback_server(
    port: u16,
    success_html: String,
    error_html: String,
) -> Result<(tokio::task::JoinHandle<()>, tokio::sync::oneshot::Receiver<std::result::Result<(String, String), String>>)> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    let handle = tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
            log::info!("[OAuth Server] Listening on http://127.0.0.1:{}/callback", port);
            
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0; 4096];
                if let Ok(n) = socket.read(&mut buffer).await {
                    let request = String::from_utf8_lossy(&buffer[..n]);
                    log::debug!("[OAuth Server] Received request");
                    
                    // Extract the authorization code and state from the query string
                    let code = extract_query_param(&request, "code");
                    let state = extract_query_param(&request, "state");

                    if let (Some(code), Some(state)) = (code, state) {
                        log::info!("[OAuth Server] Extracted authorization code");
                        
                        // Send success response
                        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", 
                            success_html.len(), success_html);
                        let _ = socket.write_all(response.as_bytes()).await;
                        
                        let _ = tx.send(Ok((code, state)));
                    } else if request.contains("code=") {
                        let response = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", 
                            error_html.len(), error_html);
                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = tx.send(Err("OAuth state parameter missing or invalid".to_string()));
                    } else {
                        let response = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", 
                            error_html.len(), error_html);
                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = tx.send(Err("No authorization code received".to_string()));
                    }
                }
            }
        }
    });
    
    Ok((handle, rx))
}

fn extract_query_param(request: &str, key: &str) -> Option<String> {
    let needle = format!("{}=", key);
    let start = request.find(&needle)? + needle.len();
    let end = request[start..]
        .find('&')
        .map(|i| start + i)
        .unwrap_or_else(|| request[start..].find(' ').map(|i| start + i).unwrap_or(request.len()));
    Some(request[start..end].to_string())
}
