//! Doppler remote source provider.
//!
//! Doppler is a secrets management platform that provides a simple API for
//! managing environment variables across projects and environments.
//!
//! ## Authentication
//!
//! Doppler supports two types of tokens:
//! - **Service Token**: Auto-scoped to a specific project/config. No scope selection needed.
//! - **Personal Token**: Requires selecting project and config.
//!
//! The token can be provided via:
//! - `DOPPLER_TOKEN` environment variable
//! - Config credential
//!
//! ## API Reference
//!
//! - [Doppler API Docs](https://docs.doppler.com/reference/api)

use crate::error::SourceError;
use crate::source::remote::traits::{
    AuthConfig, AuthField, AuthStatus, ProviderConfig, RemoteProviderInfo, RemoteSource,
    ScopeLevel, ScopeOption, ScopeSelection,
};
use crate::source::remote::RemoteSourceFactory;
use crate::source::traits::{SourceCapabilities, SourceId, SourceSnapshot};
use crate::source::variable::{ParsedVariable, VariableSource};
use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

const DOPPLER_API_BASE: &str = "https://api.doppler.com";

/// Doppler remote source implementation.
pub struct DopplerSource {
    client: Client,
    token: RwLock<Option<String>>,
    /// Cached token type (service vs personal)
    token_type: RwLock<Option<DopplerTokenType>>,
    /// Cached service token metadata (for auto-scoped tokens)
    service_token_meta: RwLock<Option<ServiceTokenMeta>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DopplerTokenType {
    /// Service token: auto-scoped to project/config
    Service,
    /// Personal token: requires scope selection
    Personal,
}

#[derive(Debug, Clone, Deserialize)]
struct ServiceTokenMeta {
    project: String,
    config: String,
}

impl DopplerSource {
    /// Creates a new Doppler source.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            token: RwLock::new(None),
            token_type: RwLock::new(None),
            service_token_meta: RwLock::new(None),
        }
    }

    /// Creates a new Doppler source with a token.
    pub fn with_token(token: impl Into<String>) -> Self {
        let source = Self::new();
        *source.token.write() = Some(token.into());
        source
    }

    /// Creates from provider config.
    pub fn from_config(config: &ProviderConfig) -> Result<Self, SourceError> {
        let source = Self::new();

        // Try to get token from config, then from env
        let token = config
            .get_string("token")
            .map(|s| s.to_string())
            .or_else(|| std::env::var("DOPPLER_TOKEN").ok());

        if let Some(t) = token {
            *source.token.write() = Some(t);
        }

        Ok(source)
    }

    fn get_token(&self) -> Option<String> {
        self.token.read().clone()
    }

    async fn detect_token_type(&self, token: &str) -> Result<DopplerTokenType, SourceError> {
        // Try the service token endpoint first
        let response = self
            .client
            .get(format!("{}/v3/me", DOPPLER_API_BASE))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| SourceError::Connection {
                provider: "doppler".into(),
                reason: e.to_string(),
            })?;

        if response.status().is_success() {
            let body: serde_json::Value = response.json().await.map_err(|e| SourceError::Remote {
                provider: "doppler".into(),
                reason: format!("Failed to parse response: {}", e),
            })?;

            // Service tokens have workplace info, personal tokens have user info
            if body.get("workplace").is_some() && body.get("config").is_some() {
                // This is a service token
                if let (Some(project), Some(config)) = (
                    body.get("project").and_then(|p| p.as_str()),
                    body.get("config").and_then(|c| c.as_str()),
                ) {
                    *self.service_token_meta.write() = Some(ServiceTokenMeta {
                        project: project.to_string(),
                        config: config.to_string(),
                    });
                    return Ok(DopplerTokenType::Service);
                }
            }

            Ok(DopplerTokenType::Personal)
        } else if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            Err(SourceError::Authentication {
                source_name: "doppler".into(),
            })
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(SourceError::Remote {
                provider: "doppler".into(),
                reason: format!("API error {}: {}", status, body),
            })
        }
    }
}

impl Default for DopplerSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RemoteSource for DopplerSource {
    fn id(&self) -> SourceId {
        SourceId::new("doppler")
    }

    fn provider_info(&self) -> RemoteProviderInfo {
        RemoteProviderInfo {
            id: "doppler".into(),
            display_name: "Doppler".into(),
            short_name: "DPL".into(),
            description: Some("Doppler secrets manager".into()),
            docs_url: Some("https://docs.doppler.com".into()),
        }
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::ASYNC_ONLY
            | SourceCapabilities::SECRETS
            | SourceCapabilities::CACHEABLE
            | SourceCapabilities::READ
    }

    fn auth_fields(&self) -> Vec<AuthField> {
        vec![AuthField {
            name: "token".into(),
            label: "Doppler Token".into(),
            description: Some("Service token or personal token".into()),
            required: true,
            secret: true,
            env_var: Some("DOPPLER_TOKEN".into()),
            default: None,
        }]
    }

    async fn auth_status(&self) -> AuthStatus {
        let token = match self.get_token() {
            Some(t) => t,
            None => return AuthStatus::NotAuthenticated,
        };

        // Check if we have cached token type
        if self.token_type.read().is_some() {
            return AuthStatus::Authenticated {
                identity: None,
                expires_at: None,
            };
        }

        // Detect token type
        match self.detect_token_type(&token).await {
            Ok(token_type) => {
                *self.token_type.write() = Some(token_type);
                AuthStatus::Authenticated {
                    identity: match token_type {
                        DopplerTokenType::Service => Some("service-token".into()),
                        DopplerTokenType::Personal => Some("personal-token".into()),
                    },
                    expires_at: None,
                }
            }
            Err(SourceError::Authentication { .. }) => AuthStatus::Failed {
                reason: "Invalid or expired token".into(),
            },
            Err(e) => AuthStatus::Failed {
                reason: e.to_string().into(),
            },
        }
    }

    async fn authenticate(&self, config: &AuthConfig) -> Result<(), SourceError> {
        let token = config.get("token").ok_or_else(|| SourceError::Remote {
            provider: "doppler".into(),
            reason: "Token is required".into(),
        })?;

        // Validate the token by detecting its type
        let token_type = self.detect_token_type(token).await?;

        *self.token.write() = Some(token.to_string());
        *self.token_type.write() = Some(token_type);

        Ok(())
    }

    fn scope_levels(&self) -> Vec<ScopeLevel> {
        // For service tokens, these are auto-filled
        // For personal tokens, these need to be selected
        vec![
            ScopeLevel {
                name: "project".into(),
                display_name: "Project".into(),
                required: true,
                multi_select: false,
                description: Some("Doppler project".into()),
            },
            ScopeLevel {
                name: "config".into(),
                display_name: "Config".into(),
                required: true,
                multi_select: false,
                description: Some("Environment config (e.g., dev, staging, prod)".into()),
            },
        ]
    }

    async fn list_options(
        &self,
        level: &str,
        parent: &ScopeSelection,
    ) -> Result<Vec<ScopeOption>, SourceError> {
        let token = self.get_token().ok_or_else(|| SourceError::Authentication {
            source_name: "doppler".into(),
        })?;

        match level {
            "project" => {
                // List all projects
                let response = self
                    .client
                    .get(format!("{}/v3/projects", DOPPLER_API_BASE))
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| SourceError::Connection {
                        provider: "doppler".into(),
                        reason: e.to_string(),
                    })?;

                if !response.status().is_success() {
                    return Err(handle_doppler_error(response).await);
                }

                #[derive(Deserialize)]
                struct ProjectsResponse {
                    projects: Vec<Project>,
                }
                #[derive(Deserialize)]
                struct Project {
                    id: String,
                    name: String,
                    description: Option<String>,
                }

                let body: ProjectsResponse =
                    response.json().await.map_err(|e| SourceError::Remote {
                        provider: "doppler".into(),
                        reason: format!("Failed to parse projects: {}", e),
                    })?;

                Ok(body
                    .projects
                    .into_iter()
                    .map(|p| ScopeOption {
                        id: p.id.into(),
                        display_name: p.name.into(),
                        description: p.description.map(|d| d.into()),
                        icon: None,
                    })
                    .collect())
            }
            "config" => {
                // List configs for the selected project
                let project = parent.get_single("project").ok_or_else(|| {
                    SourceError::InvalidScope {
                        provider: "doppler".into(),
                        reason: "Project must be selected first".into(),
                    }
                })?;

                let response = self
                    .client
                    .get(format!("{}/v3/configs", DOPPLER_API_BASE))
                    .query(&[("project", project)])
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| SourceError::Connection {
                        provider: "doppler".into(),
                        reason: e.to_string(),
                    })?;

                if !response.status().is_success() {
                    return Err(handle_doppler_error(response).await);
                }

                #[derive(Deserialize)]
                struct ConfigsResponse {
                    configs: Vec<Config>,
                }
                #[derive(Deserialize)]
                struct Config {
                    name: String,
                    root: bool,
                    locked: bool,
                }

                let body: ConfigsResponse =
                    response.json().await.map_err(|e| SourceError::Remote {
                        provider: "doppler".into(),
                        reason: format!("Failed to parse configs: {}", e),
                    })?;

                Ok(body
                    .configs
                    .into_iter()
                    .map(|c| {
                        let mut desc = String::new();
                        if c.root {
                            desc.push_str("Root config");
                        }
                        if c.locked {
                            if !desc.is_empty() {
                                desc.push_str(", ");
                            }
                            desc.push_str("Locked");
                        }
                        ScopeOption {
                            id: c.name.clone().into(),
                            display_name: c.name.into(),
                            description: if desc.is_empty() {
                                None
                            } else {
                                Some(desc.into())
                            },
                            icon: if c.locked {
                                Some("🔒".into())
                            } else {
                                None
                            },
                        }
                    })
                    .collect())
            }
            _ => Err(SourceError::InvalidScope {
                provider: "doppler".into(),
                reason: format!("Unknown scope level: {}", level),
            }),
        }
    }

    async fn fetch_secrets(&self, scope: &ScopeSelection) -> Result<SourceSnapshot, SourceError> {
        let token = self.get_token().ok_or_else(|| SourceError::Authentication {
            source_name: "doppler".into(),
        })?;

        // Determine project and config
        let (project, config) = {
            // Check if we have a service token with auto-scoped metadata
            if let Some(meta) = self.service_token_meta.read().as_ref() {
                (meta.project.clone(), meta.config.clone())
            } else {
                // Use scope selection for personal tokens
                let project =
                    scope
                        .get_single("project")
                        .ok_or_else(|| SourceError::InvalidScope {
                            provider: "doppler".into(),
                            reason: "Project is required".into(),
                        })?;
                let config =
                    scope
                        .get_single("config")
                        .ok_or_else(|| SourceError::InvalidScope {
                            provider: "doppler".into(),
                            reason: "Config is required".into(),
                        })?;
                (project.to_string(), config.to_string())
            }
        };

        // Fetch secrets
        let response = self
            .client
            .get(format!("{}/v3/configs/config/secrets", DOPPLER_API_BASE))
            .query(&[("project", &project), ("config", &config)])
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SourceError::Connection {
                provider: "doppler".into(),
                reason: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(handle_doppler_error(response).await);
        }

        #[derive(Deserialize)]
        struct SecretsResponse {
            secrets: std::collections::HashMap<String, SecretValue>,
        }
        #[derive(Deserialize)]
        struct SecretValue {
            raw: String,
            #[serde(default)]
            note: Option<String>,
        }

        let body: SecretsResponse = response.json().await.map_err(|e| SourceError::Remote {
            provider: "doppler".into(),
            reason: format!("Failed to parse secrets: {}", e),
        })?;

        // Convert to ParsedVariables
        let variables: Vec<ParsedVariable> = body
            .secrets
            .into_iter()
            .map(|(key, value)| ParsedVariable {
                key: key.into(),
                raw_value: value.raw.into(),
                source: VariableSource::Remote {
                    provider: "doppler".into(),
                    path: Some(format!("{}/{}", project, config)),
                },
                description: value.note.map(|n| n.into()),
                is_commented: false,
            })
            .collect();

        Ok(SourceSnapshot {
            source_id: self.id(),
            variables: Arc::from(variables),
            timestamp: Instant::now(),
            version: None,
        })
    }
}

async fn handle_doppler_error(response: reqwest::Response) -> SourceError {
    let status = response.status();

    // Check for rate limiting
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        return SourceError::RateLimited {
            provider: "doppler".into(),
            retry_after_secs: retry_after,
        };
    }

    // Check for auth errors
    if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        return SourceError::Authentication {
            source_name: "doppler".into(),
        };
    }

    // Try to get error message from body
    let body = response.text().await.unwrap_or_default();

    #[derive(Deserialize)]
    struct ErrorResponse {
        messages: Option<Vec<String>>,
        message: Option<String>,
    }

    let reason = if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
        err.messages
            .and_then(|m| m.first().cloned())
            .or(err.message)
            .unwrap_or_else(|| format!("HTTP {}", status))
    } else {
        format!("HTTP {}: {}", status, body)
    };

    SourceError::Remote {
        provider: "doppler".into(),
        reason,
    }
}

/// Factory for creating Doppler sources.
pub struct DopplerSourceFactory;

#[async_trait]
impl RemoteSourceFactory for DopplerSourceFactory {
    fn provider_id(&self) -> &str {
        "doppler"
    }

    fn provider_name(&self) -> &str {
        "Doppler"
    }

    async fn create(&self, config: &ProviderConfig) -> Result<Arc<dyn RemoteSource>, SourceError> {
        Ok(Arc::new(DopplerSource::from_config(config)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doppler_source_creation() {
        let source = DopplerSource::new();
        assert_eq!(source.id().as_str(), "doppler");
        assert_eq!(source.provider_info().display_name.as_str(), "Doppler");
    }

    #[test]
    fn test_scope_levels() {
        let source = DopplerSource::new();
        let levels = source.scope_levels();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].name.as_str(), "project");
        assert_eq!(levels[1].name.as_str(), "config");
    }

    #[test]
    fn test_auth_fields() {
        let source = DopplerSource::new();
        let fields = source.auth_fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.as_str(), "token");
        assert!(fields[0].secret);
    }
}
