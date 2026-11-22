use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    routing::post,
    Json, Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use validator::Validate;

use crate::state::AppState;

#[derive(Debug, Deserialize, Validate)]
struct RegisterRequest {
    #[validate(email)]
    email: String,
    #[validate(length(min = 2, max = 80))]
    full_name: String,
    #[validate(length(min = 8, max = 128))]
    password: String,
}

#[derive(Debug, Deserialize, Validate)]
struct LoginRequest {
    #[validate(email)]
    email: String,
    #[validate(length(min = 1, max = 128))]
    password: String,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: uuid::Uuid,
    email: String,
    full_name: String,
    avatar_url: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
    token_type: &'static str,
    expires_in: i64,
    user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    error: &'static str,
}

#[derive(Debug, FromRow)]
struct UserWithPassword {
    id: uuid::Uuid,
    email: String,
    full_name: String,
    avatar_url: Option<String>,
    password_hash: String,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct UserWithoutPassword {
    id: uuid::Uuid,
    email: String,
    full_name: String,
    avatar_url: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    email: String,
    exp: usize,
    iat: usize,
}

/// User identity extracted from a verified Authorization: Bearer token.
pub struct AuthUser(pub uuid::Uuid);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, Json<ApiError>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    error: "Authentication is required",
                }),
            ))?;

        let (user_id, _) = verify_token(token, &state.jwt_secret).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    error: "Your session is invalid or has expired",
                }),
            )
        })?;

        Ok(Self(user_id))
    }
}

/// Verify a bearer JWT and return the authenticated user ID.
pub fn verify_token(token: &str, secret: &str) -> Result<(uuid::Uuid, usize), StatusCode> {
    let decoded = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = uuid::Uuid::parse_str(&decoded.claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok((user_id, decoded.claims.exp))
}

fn issue_token(user_id: uuid::Uuid, email: &str, secret: &str) -> Result<(String, i64), StatusCode> {
    let now = Utc::now();
    let expires_in = Duration::hours(24).num_seconds();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_owned(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::seconds(expires_in)).timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((token, expires_in))
}

fn invalid_request() -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: "Please provide valid account details",
        }),
    )
}

async fn register(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), (StatusCode, Json<ApiError>)> {
    request.validate().map_err(|_| invalid_request())?;

    let email = request.email.trim().to_lowercase();
    let full_name = request.full_name.trim().to_owned();
    if full_name.chars().count() < 2 {
        return Err(invalid_request());
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(request.password.as_bytes(), &salt)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Unable to create account",
                }),
            )
        })?
        .to_string();

    let user = sqlx::query_as::<_, UserWithoutPassword>(
        "INSERT INTO users (email, password_hash, full_name)          VALUES ($1, $2, $3)          RETURNING id, email, full_name, avatar_url, created_at",
    )
    .bind(&email)
    .bind(password_hash)
    .bind(&full_name)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        if let sqlx::Error::Database(database_error) = &error {
            if database_error.code().as_deref() == Some("23505") {
                return (
                    StatusCode::CONFLICT,
                    Json(ApiError {
                        error: "An account with this email already exists",
                    }),
                );
            }
        }
        tracing::error!(%error, "Account registration failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Unable to create account",
            }),
        )
    })?;

    let (token, expires_in) = issue_token(user.id, &user.email, &state.jwt_secret)
        .map_err(|status| {
            (
                status,
                Json(ApiError {
                    error: "Unable to create session",
                }),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            token_type: "Bearer",
            expires_in,
            user: UserResponse {
                id: user.id,
                email: user.email,
                full_name: user.full_name,
                avatar_url: user.avatar_url,
                created_at: user.created_at,
            },
        }),
    ))
}

async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ApiError>)> {
    request.validate().map_err(|_| invalid_request())?;

    let email = request.email.trim().to_lowercase();
    let user = sqlx::query_as::<_, UserWithPassword>(
        "SELECT id, email, full_name, avatar_url, password_hash, created_at          FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "Account lookup failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Unable to sign in",
            }),
        )
    })?;

    let Some(user) = user else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "Invalid email or password",
            }),
        ));
    };

    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|_| {
        tracing::error!(user_id = %user.id, "Stored password hash is invalid");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Unable to sign in",
            }),
        )
    })?;

    if Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "Invalid email or password",
            }),
        ));
    }

    let (token, expires_in) = issue_token(user.id, &user.email, &state.jwt_secret)
        .map_err(|status| {
            (
                status,
                Json(ApiError {
                    error: "Unable to create session",
                }),
            )
        })?;

    Ok(Json(AuthResponse {
        token,
        token_type: "Bearer",
        expires_in,
        user: UserResponse {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            avatar_url: user.avatar_url,
            created_at: user.created_at,
        },
    }))
}

/// Public authentication endpoints; mount this router under /api/auth.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_hs256_token_verifies_user_and_expiration() {
        let user_id = uuid::Uuid::new_v4();
        let secret = "test-only-secret-with-more-than-32-characters";
        let (token, lifetime) = issue_token(user_id, "student@example.edu", secret).unwrap();

        let (verified_user, expires_at) = verify_token(&token, secret).unwrap();

        assert_eq!(verified_user, user_id);
        assert!(lifetime > 0);
        assert!(expires_at > Utc::now().timestamp() as usize);
    }

    #[test]
    fn token_with_a_modified_signature_is_rejected() {
        let user_id = uuid::Uuid::new_v4();
        let secret = "test-only-secret-with-more-than-32-characters";
        let (mut token, _) = issue_token(user_id, "student@example.edu", secret).unwrap();

        let last = token.pop().unwrap();
        token.push(if last == 'a' { 'b' } else { 'a' });

        assert_eq!(verify_token(&token, secret), Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn token_signed_with_a_different_secret_is_rejected() {
        let user_id = uuid::Uuid::new_v4();
        let (token, _) = issue_token(
            user_id,
            "student@example.edu",
            "original-test-secret-with-more-than-32-chars",
        )
        .unwrap();

        assert_eq!(
            verify_token(&token, "different-test-secret-with-more-than-32-chars"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }
}

