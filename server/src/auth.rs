use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use validator::Validate;
use sha2::{Digest, Sha256};

use crate::state::AppState;

#[derive(Debug, Deserialize, Validate)]
struct LoginRequest {
    #[validate(email)]
    email: String,
    #[validate(length(min = 1, max = 128))]
    password: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserResponse {
    pub(crate) id: uuid::Uuid,
    pub(crate) email: String,
    pub(crate) full_name: String,
    pub(crate) avatar_url: Option<String>,
    pub(crate) created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthResponse {
    pub(crate) expires_in: i64,
    pub(crate) user: UserResponse,
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
    password_hash: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    email: String,
    exp: usize,
    iat: usize,
    #[serde(default)]
    jti: String,
}

const SESSION_COOKIE_NAME: &str = "campusforge_session";

/// Read the HttpOnly session cookie from a request. The JWT is never exposed to browser JavaScript.
pub(crate) fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == SESSION_COOKIE_NAME && !value.is_empty()).then(|| value.to_owned())
    })
}

/// Require unsafe cookie-authenticated requests to originate from the configured frontend.
pub(crate) fn frontend_origin_matches(headers: &HeaderMap) -> bool {
    let expected = std::env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_owned());
    let expected = expected.trim_end_matches('/');
    headers.get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| origin == expected)
}

fn cookie_security_attributes() -> &'static str {
    let secure = std::env::var("COOKIE_SECURE")
        .ok()
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or_else(|| {
            std::env::var("FRONTEND_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".to_owned())
                .starts_with("https://")
        });
    if secure { "; Secure; SameSite=None" } else { "; SameSite=Lax" }
}

pub(crate) fn session_cookie_header(token: &str, expires_in: i64) -> Result<HeaderValue, StatusCode> {
    let cookie = format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; Max-Age={}{attributes}",
        expires_in.max(0),
        attributes = cookie_security_attributes()
    );
    HeaderValue::from_str(&cookie).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(crate) fn clear_session_cookie_header() -> HeaderValue {
    let cookie = format!(
        "{SESSION_COOKIE_NAME}=; Path=/; HttpOnly; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT{}",
        cookie_security_attributes()
    );
    HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(
        "campusforge_session=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax"
    ))
}

/// User identity extracted from a verified bearer token or HttpOnly session cookie.
pub struct AuthUser(pub uuid::Uuid);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, Json<ApiError>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let bearer = parts.headers.get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));
        let (token, from_cookie) = match bearer {
            Some(token) => (token.to_owned(), false),
            None => (
                session_token_from_headers(&parts.headers).ok_or((
                    StatusCode::UNAUTHORIZED,
                    Json(ApiError { error: "Authentication is required" }),
                ))?,
                true,
            ),
        };

        let is_safe_method = matches!(parts.method.as_str(), "GET" | "HEAD" | "OPTIONS");
        if from_cookie && !is_safe_method && !frontend_origin_matches(&parts.headers) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ApiError { error: "The request origin is not allowed" }),
            ));
        }

        let (user_id, _, session_id) =
            verify_session_token(&token, &state.jwt_secret).map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        error: "Your session is invalid or has expired",
                    }),
                )
            })?;

        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM auth_sessions
                 WHERE id=$1 AND user_id=$2
                   AND revoked_at IS NULL AND expires_at > NOW()
            )",
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not validate authentication session");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError {
                    error: "Authentication is temporarily unavailable",
                }),
            )
        })?;

        if !active {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    error: "Your session is invalid or has expired",
                }),
            ));
        }

        Ok(Self(user_id))
    }
}

/// Board viewers may read board resources but cannot make changes.
/// Workspace membership alone never grants board access.
fn logout_request_allowed(
    has_cookie: bool,
    has_bearer: bool,
    valid_bearer: bool,
    trusted_origin: bool,
) -> bool {
    if has_cookie || !has_bearer {
        trusted_origin
    } else {
        valid_bearer
    }
}

pub(crate) fn board_role_allows(role: Option<&str>, write: bool) -> bool {
    match (role, write) {
        (Some("admin" | "member" | "viewer"), false) => true,
        (Some("admin" | "member"), true) => true,
        _ => false,
    }
}

/// Return the caller's role on a board in the selected workspace.
/// Workspace membership alone is not sufficient for board-scoped reads or writes.
pub(crate) async fn board_role(
    pool: &PgPool,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    board_id: uuid::Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT bm.role
         FROM board_members bm
         JOIN boards b ON b.id = bm.board_id
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = bm.user_id
         WHERE bm.user_id = $1 AND b.workspace_id = $2 AND b.id = $3",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(board_id)
    .fetch_optional(pool)
    .await
}

/// Authenticated user scoped to one explicitly selected workspace.
/// Requests without a workspace header are accepted only when the account has
/// exactly one workspace membership.
pub struct WorkspaceUser(pub uuid::Uuid, pub uuid::Uuid);

impl FromRequestParts<AppState> for WorkspaceUser {
    type Rejection = (StatusCode, Json<ApiError>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user_id) = AuthUser::from_request_parts(parts, state).await?;
        let requested_workspace = parts.headers.get("x-workspace-id")
            .map(|value| value.to_str().map_err(|_| {
                (StatusCode::BAD_REQUEST, Json(ApiError {
                    error: "Workspace selection is invalid",
                }))
            }))
            .transpose()?
            .map(|value| uuid::Uuid::parse_str(value).map_err(|_| {
                (StatusCode::BAD_REQUEST, Json(ApiError {
                    error: "Workspace selection is invalid",
                }))
            }))
            .transpose()?;

        let workspace_id = match requested_workspace {
            Some(workspace_id) => workspace_id,
            None => {
                let memberships = sqlx::query_scalar::<_, uuid::Uuid>(
                    "SELECT workspace_id FROM workspace_members
                     WHERE user_id=$1 ORDER BY joined_at DESC LIMIT 2",
                )
                .bind(user_id)
                .fetch_all(&state.db)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
                    error: "Could not resolve the selected workspace",
                })))?;
                match memberships.as_slice() {
                    [workspace_id] => *workspace_id,
                    [] => return Err((StatusCode::NOT_FOUND, Json(ApiError {
                        error: "Workspace not found",
                    }))),
                    _ => return Err((StatusCode::BAD_REQUEST, Json(ApiError {
                        error: "Select a workspace before continuing",
                    }))),
                }
            }
        };

        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspace_members
             WHERE user_id=$1 AND workspace_id=$2)",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            error: "Could not verify workspace membership",
        })))?;

        if !is_member {
            return Err((StatusCode::FORBIDDEN, Json(ApiError {
                error: "You are not a member of the selected workspace",
            })));
        }

        Ok(Self(user_id, workspace_id))
    }
}

/// Decode a signed JWT. Tokens without a session identifier remain decodable for
/// diagnostics, but are rejected by verify_session_token after revocation is enabled.
fn decode_claims(token: &str, secret: &str) -> Result<Claims, StatusCode> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|decoded| decoded.claims)
    .map_err(|_| StatusCode::UNAUTHORIZED)
}

/// Verify a bearer JWT and return the authenticated user ID and expiry.
pub fn verify_token(token: &str, secret: &str) -> Result<(uuid::Uuid, usize), StatusCode> {
    let claims = decode_claims(token, secret)?;
    let user_id = uuid::Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok((user_id, claims.exp))
}

/// Verify a JWT and require its JTI to be a valid server-recorded session ID.
pub(crate) fn verify_session_token(
    token: &str,
    secret: &str,
) -> Result<(uuid::Uuid, usize, uuid::Uuid), StatusCode> {
    let claims = decode_claims(token, secret)?;
    let user_id = uuid::Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let session_id = uuid::Uuid::parse_str(&claims.jti).map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok((user_id, claims.exp, session_id))
}

/// Persist only the non-secret session ID and lifecycle metadata, never the JWT.
pub(crate) async fn persist_session(
    db: &PgPool,
    user_id: uuid::Uuid,
    session_id: uuid::Uuid,
    expires_at: usize,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, expires_at)
         VALUES ($1, $2, to_timestamp($3::bigint::double precision))",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(expires_at as i64)
    .execute(db)
    .await?;
    Ok(())
}

pub(crate) async fn run_session_cleanup(db: PgPool) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        if let Err(error) = sqlx::query(
            "DELETE FROM auth_sessions
             WHERE expires_at < NOW() - INTERVAL '7 days'
                OR revoked_at < NOW() - INTERVAL '7 days'",
        )
        .execute(&db)
        .await
        {
            tracing::warn!(%error, "Could not clean expired authentication sessions");
        }
    }
}

pub(crate) fn issue_token(
    user_id: uuid::Uuid,
    email: &str,
    secret: &str,
) -> Result<(String, i64, uuid::Uuid, usize), StatusCode> {
    let now = Utc::now();
    let expires_in = Duration::hours(24).num_seconds();
    let expires_at = (now + Duration::seconds(expires_in)).timestamp() as usize;
    let session_id = uuid::Uuid::new_v4();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_owned(),
        iat: now.timestamp() as usize,
        exp: expires_at,
        jti: session_id.to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((token, expires_in, session_id, expires_at))
}


fn rate_limit_digest(scope: &str, value: &str) -> String {
    let digest = Sha256::digest(format!("{scope}:{value}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn client_ip(headers: &HeaderMap) -> String {
    headers.get("x-real-ip").or_else(|| headers.get("x-forwarded-for"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 64)
        .unwrap_or("unknown")
        .to_owned()
}

fn rate_limited() -> (StatusCode, Json<ApiError>) {
    (StatusCode::TOO_MANY_REQUESTS, Json(ApiError { error: "Too many sign-in attempts. Please wait before trying again." }))
}

fn invalid_request() -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: "Please provide valid account details",
        }),
    )
}

async fn register() -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::FORBIDDEN,
        Json(ApiError {
            error: "New accounts must sign up with Google, GitHub, or Discord using a verified email address",
        }),
    )
}

async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    request.validate().map_err(|_| invalid_request())?;

    let email = request.email.trim().to_lowercase();
    let ip_key = rate_limit_digest("login-ip", &client_ip(&headers));
    let email_key = rate_limit_digest("login-email", &email);
    let ip_allowed = state.check_rate_limit(&ip_key, 30, 900).await.map_err(|error| {
        tracing::error!(%error, "Could not enforce login IP rate limit");
        (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError { error: "Sign-in is temporarily unavailable" }))
    })?;
    let email_allowed = state.check_rate_limit(&email_key, 10, 900).await.map_err(|error| {
        tracing::error!(%error, "Could not enforce login account rate limit");
        (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError { error: "Sign-in is temporarily unavailable" }))
    })?;
    if !ip_allowed || !email_allowed { return Err(rate_limited()); }

    let user = sqlx::query_as::<_, UserWithPassword>(
        "SELECT id, email, full_name, avatar_url, password_hash, created_at
         FROM users WHERE email = $1",
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

    let Some(password_hash) = user.password_hash.as_deref() else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "Invalid email or password",
            }),
        ));
    };

    let parsed_hash = PasswordHash::new(password_hash).map_err(|_| {
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

    let (token, expires_in, session_id, expires_at) =
        issue_token(user.id, &user.email, &state.jwt_secret).map_err(|status| {
            (
                status,
                Json(ApiError {
                    error: "Unable to create session",
                }),
            )
        })?;
    persist_session(&state.db, user.id, session_id, expires_at)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not persist authentication session");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError {
                    error: "Sign-in is temporarily unavailable",
                }),
            )
        })?;

    let cookie = session_cookie_header(&token, expires_in).map_err(|status| {
        (status, Json(ApiError { error: "Unable to create session" }))
    })?;
    let mut response = Json(AuthResponse {
        expires_in,
        user: UserResponse {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            avatar_url: user.avatar_url,
            created_at: user.created_at,
        },
    }).into_response();
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}

fn session_or_bearer_token_from_headers(headers: &HeaderMap) -> Option<(String, bool)> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| (token.to_owned(), false))
        .or_else(|| session_token_from_headers(headers).map(|token| (token, true)))
}

async fn logout(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let cookie_token = session_token_from_headers(&headers);

    // A request may carry two distinct sessions. Revoke both before clearing the
    // cookie; a bad bearer must never suppress revocation of a valid cookie.
    let valid_bearer = bearer_token
        .and_then(|token| verify_session_token(token, &state.jwt_secret).ok())
        .is_some();
    if !logout_request_allowed(
        cookie_token.is_some(),
        bearer_token.is_some(),
        valid_bearer,
        frontend_origin_matches(&headers),
    ) {
        return Err(if cookie_token.is_some() || bearer_token.is_none() {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::UNAUTHORIZED
        });
    }

    let mut tokens = Vec::new();
    if let Some(token) = bearer_token { tokens.push(token.to_owned()); }
    if let Some(token) = cookie_token {
        if !tokens.iter().any(|existing| existing == &token) {
            tokens.push(token);
        }
    }

    for token in tokens {
        if let Ok((user_id, _, session_id)) = verify_session_token(&token, &state.jwt_secret) {
            sqlx::query(
                "UPDATE auth_sessions
                    SET revoked_at = COALESCE(revoked_at, NOW())
                  WHERE id=$1 AND user_id=$2",
            )
            .bind(session_id)
            .bind(user_id)
            .execute(&state.db)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Could not revoke authentication session");
                StatusCode::SERVICE_UNAVAILABLE
            })?;
        }
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().append(header::SET_COOKIE, clear_session_cookie_header());
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn board_role_lookup_uses_real_workspace_and_board_memberships() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!("Skipping board role integration test: DATABASE_URL is not configured");
            return;
        };
        let pool = sqlx::PgPool::connect(&database_url).await
            .expect("test PostgreSQL service should accept connections");
        let user_id = uuid::Uuid::new_v4();
        let workspace_id = uuid::Uuid::new_v4();
        let board_id = uuid::Uuid::new_v4();
        let email = format!("{}@example.test", uuid::Uuid::new_v4());
        let provider_user_id = format!("board-role-{}", uuid::Uuid::new_v4());

        let mut tx = pool.begin().await.expect("fixture transaction should start");
        sqlx::query(
            "INSERT INTO users(id,email,password_hash,full_name)
             VALUES($1,$2,NULL,'Board Role Test User')",
        ).bind(user_id).bind(&email).execute(&mut *tx).await
            .expect("OAuth fixture user should be inserted");
        sqlx::query(
            "INSERT INTO oauth_accounts(provider,provider_user_id,user_id,email)
             VALUES('google',$1,$2,$3)",
        ).bind(provider_user_id).bind(user_id).bind(&email).execute(&mut *tx).await
            .expect("verified provider identity should be attached");
        sqlx::query("INSERT INTO workspaces(id,name,owner_id) VALUES($1,'Board Role Test',$2)")
            .bind(workspace_id).bind(user_id).execute(&mut *tx).await
            .expect("workspace fixture should be inserted");
        sqlx::query("INSERT INTO workspace_members(workspace_id,user_id,role) VALUES($1,$2,'owner')")
            .bind(workspace_id).bind(user_id).execute(&mut *tx).await
            .expect("workspace owner membership should be inserted");
        sqlx::query("INSERT INTO boards(id,workspace_id,name,created_by) VALUES($1,$2,'Role Test',$3)")
            .bind(board_id).bind(workspace_id).bind(user_id).execute(&mut *tx).await
            .expect("board fixture should be inserted");
        sqlx::query("INSERT INTO board_members(board_id,user_id,role) VALUES($1,$2,'admin')")
            .bind(board_id).bind(user_id).execute(&mut *tx).await
            .expect("board administrator membership should be inserted");
        tx.commit().await.expect("board role fixture should commit");

        assert_eq!(board_role(&pool,user_id,workspace_id,board_id).await.unwrap().as_deref(),Some("admin"));
        sqlx::query("UPDATE board_members SET role='viewer' WHERE board_id=$1 AND user_id=$2")
            .bind(board_id).bind(user_id).execute(&pool).await
            .expect("board role should be changeable");
        let role = board_role(&pool,user_id,workspace_id,board_id).await.unwrap();
        assert_eq!(role.as_deref(),Some("viewer"));
        assert!(board_role_allows(role.as_deref(),false));
        assert!(!board_role_allows(role.as_deref(),true));

        sqlx::query("DELETE FROM workspaces WHERE id=$1").bind(workspace_id).execute(&pool).await
            .expect("workspace fixture should be removed");
        sqlx::query("DELETE FROM users WHERE id=$1").bind(user_id).execute(&pool).await
            .expect("OAuth fixture user should be removed");
        pool.close().await;
    }

    #[test]
    fn cookie_logout_requires_trusted_origin_even_with_a_bearer() {
        assert!(!logout_request_allowed(true, true, true, false));
        assert!(!logout_request_allowed(true, true, false, false));
        assert!(logout_request_allowed(true, true, true, true));
        assert!(logout_request_allowed(true, false, false, true));
        assert!(logout_request_allowed(false, true, true, false));
        assert!(!logout_request_allowed(false, true, false, true));
        assert!(logout_request_allowed(false, false, false, true));
        assert!(!logout_request_allowed(false, false, false, false));
    }

    #[test]
    fn board_roles_allow_viewers_to_read_but_not_write() {
        assert!(board_role_allows(Some("viewer"), false));
        assert!(!board_role_allows(Some("viewer"), true));
        assert!(board_role_allows(Some("member"), false));
        assert!(board_role_allows(Some("member"), true));
        assert!(board_role_allows(Some("admin"), false));
        assert!(board_role_allows(Some("admin"), true));
        assert!(!board_role_allows(None, false));
        assert!(!board_role_allows(None, true));
        assert!(!board_role_allows(Some("unknown"), false));
    }

    #[test]
    fn logout_resolves_bearer_tokens_and_cookie_sessions() {
        let mut bearer_headers = HeaderMap::new();
        bearer_headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer bearer-token"));
        bearer_headers.insert(header::COOKIE, HeaderValue::from_static("campusforge_session=cookie-token"));
        assert_eq!(
            session_or_bearer_token_from_headers(&bearer_headers),
            Some(("bearer-token".to_owned(), false))
        );

        let mut cookie_headers = HeaderMap::new();
        cookie_headers.insert(header::COOKIE, HeaderValue::from_static("campusforge_session=cookie-token"));
        assert_eq!(
            session_or_bearer_token_from_headers(&cookie_headers),
            Some(("cookie-token".to_owned(), true))
        );
    }

    #[test]
    fn issued_hs256_token_verifies_user_and_expiration() {
        let user_id = uuid::Uuid::new_v4();
        let secret = "test-only-secret-with-more-than-32-characters";
        let (token, lifetime, session_id, token_expires_at) =
            issue_token(user_id, "student@example.edu", secret).unwrap();

        let (verified_user, expires_at) = verify_token(&token, secret).unwrap();
        let (session_user, session_expiry, verified_session_id) =
            verify_session_token(&token, secret).unwrap();

        assert_eq!(verified_user, user_id);
        assert_eq!(session_user, user_id);
        assert_eq!(verified_session_id, session_id);
        assert_eq!(session_expiry, token_expires_at);
        assert!(lifetime > 0);
        assert!(expires_at > Utc::now().timestamp() as usize);
    }

    #[test]
    fn token_with_a_modified_signature_is_rejected() {
        let user_id = uuid::Uuid::new_v4();
        let secret = "test-only-secret-with-more-than-32-characters";
        let (mut token, _, _, _) = issue_token(user_id, "student@example.edu", secret).unwrap();

        let signature_start = token.rfind('.').unwrap() + 1;
        let first_signature_byte = token.as_bytes()[signature_start];
        let replacement = if first_signature_byte == b'a' { 'b' } else { 'a' };
        token.replace_range(signature_start..signature_start + 1, &replacement.to_string());

        assert_eq!(verify_token(&token, secret), Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn session_cookie_is_http_only_and_scoped_to_the_application() {
        let cookie = session_cookie_header("header.payload.signature", 3600)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=3600"));
        assert!(!cookie.contains("campusforge-auth-token"));
    }

    #[test]
    fn session_cookie_parser_reads_only_the_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; campusforge_session=header.payload.signature; other=value"),
        );

        assert_eq!(
            session_token_from_headers(&headers).as_deref(),
            Some("header.payload.signature")
        );
    }

    #[test]
    fn token_signed_with_a_different_secret_is_rejected() {
        let user_id = uuid::Uuid::new_v4();
        let (token, _, _, _) = issue_token(
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
