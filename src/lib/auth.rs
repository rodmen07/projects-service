use std::env;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;

use crate::models::ApiError;

pub const AUTH_HEADER: &str = "Authorization";
pub const AUTH_SCHEME: &str = "Bearer";

#[derive(Debug, Deserialize, Clone)]
pub struct AuthClaims {
    pub sub: String,
    #[serde(default)]
    pub roles: Vec<String>,
}

impl AuthClaims {
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case(role))
    }

    pub fn is_admin(&self) -> bool {
        self.has_role("admin")
    }

    #[allow(dead_code)]
    pub fn is_client(&self) -> bool {
        self.has_role("client")
    }

    fn dev_claims() -> Self {
        AuthClaims {
            sub: "dev-user".to_string(),
            roles: vec!["admin".to_string()],
        }
    }
}

#[derive(Debug)]
pub enum AuthError {
    MissingHeader,
    InvalidHeaderFormat,
    InvalidToken,
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingHeader => "AUTH_REQUIRED",
            Self::InvalidHeaderFormat => "AUTH_INVALID_FORMAT",
            Self::InvalidToken => "AUTH_INVALID_TOKEN",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::MissingHeader => "authorization header is required",
            Self::InvalidHeaderFormat => "authorization header must be 'Bearer <token>'",
            Self::InvalidToken => "token validation failed",
        }
    }
}

pub fn auth_enforced() -> bool {
    env::var("AUTH_ENFORCED")
        .ok()
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(true) // default true for projects-service
}

fn auth_secret() -> String {
    env::var("AUTH_JWT_SECRET").unwrap_or_else(|_| "dev-insecure-secret-change-me".to_string())
}

fn auth_algorithm() -> Algorithm {
    match env::var("AUTH_JWT_ALGORITHM")
        .unwrap_or_default()
        .trim()
        .to_uppercase()
        .as_str()
    {
        "RS256" => Algorithm::RS256,
        "RS384" => Algorithm::RS384,
        "RS512" => Algorithm::RS512,
        "HS384" => Algorithm::HS384,
        "HS512" => Algorithm::HS512,
        _ => Algorithm::HS256,
    }
}

fn auth_issuer() -> String {
    env::var("AUTH_ISSUER").unwrap_or_else(|_| "auth-service".to_string())
}

fn normalise_pem(raw: &str) -> String {
    raw.replace("\\n", "\n")
}

fn decoding_key(algorithm: Algorithm) -> Result<DecodingKey, AuthError> {
    match algorithm {
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
            let raw = env::var("AUTH_JWT_PUBLIC_KEY").map_err(|_| AuthError::InvalidToken)?;
            DecodingKey::from_rsa_pem(normalise_pem(&raw).as_bytes())
                .map_err(|_| AuthError::InvalidToken)
        }
        _ => Ok(DecodingKey::from_secret(auth_secret().as_bytes())),
    }
}

fn extract_bearer_token(header: &str) -> Result<&str, AuthError> {
    let mut parts = header.split_whitespace();
    let scheme = parts.next().ok_or(AuthError::InvalidHeaderFormat)?;
    let token = parts.next().ok_or(AuthError::InvalidHeaderFormat)?;
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case(AUTH_SCHEME) {
        return Err(AuthError::InvalidHeaderFormat);
    }
    Ok(token)
}

pub fn validate_authorization_header(header: Option<&str>) -> Result<AuthClaims, AuthError> {
    let raw = header.ok_or(AuthError::MissingHeader)?;
    let token = extract_bearer_token(raw)?;
    let algorithm = auth_algorithm();
    let mut validation = Validation::new(algorithm);
    validation.validate_exp = true;
    validation.set_issuer(&[auth_issuer()]);
    let key = decoding_key(algorithm)?;
    decode::<AuthClaims>(token, &key, &validation)
        .map(|d| d.claims)
        .map_err(|_| AuthError::InvalidToken)
}

// Axum extractor — used by handlers that need caller identity
impl<S: Send + Sync> FromRequestParts<S> for AuthClaims {
    type Rejection = (StatusCode, Json<ApiError>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTH_HEADER)
            .and_then(|v| v.to_str().ok());

        match validate_authorization_header(header) {
            Ok(claims) => Ok(claims),
            Err(e) => {
                if !auth_enforced() {
                    Ok(Self::dev_claims())
                } else {
                    Err((
                        StatusCode::UNAUTHORIZED,
                        Json(ApiError {
                            code: e.code().to_string(),
                            message: e.message().to_string(),
                            details: None,
                        }),
                    ))
                }
            }
        }
    }
}
