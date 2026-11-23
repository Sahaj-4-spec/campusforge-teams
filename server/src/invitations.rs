use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::{auth::{AuthUser, WorkspaceUser}, state::AppState};

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ErrorBody>)>;

fn error(status: StatusCode, message: &'static str) -> (StatusCode, Json<ErrorBody>) {
    (status, Json(ErrorBody { error: message }))
}

fn internal_error<E: std::fmt::Display>(cause: E, context: &'static str) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(error = %cause, "{context}");
    error(StatusCode::INTERNAL_SERVER_ERROR, "The request could not be completed")
}

fn make_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if email.len() > 254 || email.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

async fn can_manage_workspace(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM workspace_members
         WHERE user_id=$1 AND workspace_id=$2",
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(matches!(role.as_deref(), Some("owner" | "admin")))
}

async fn add_activity(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    actor_id: Uuid,
    event_type: &'static str,
    description: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_events (workspace_id, actor_id, event_type, payload)
         VALUES ($1, $2, $3, jsonb_build_object('description', $4))",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .bind(event_type)
    .bind(description)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CreateInvitationInput {
    email: String,
    role: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
struct InvitationRow {
    id: Uuid,
    email: String,
    role: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct CreatedInvitation {
    id: Uuid,
    email: String,
    role: String,
    expires_at: DateTime<Utc>,
    token: String,
}

async fn list_invitations(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<Vec<InvitationRow>>> {
    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin invitation lookup"))?;
    let can_manage = can_manage_workspace(&mut tx, user_id, workspace_id)
        .await
        .map_err(|cause| internal_error(cause, "Could not verify invitation permissions"))?;
    if !can_manage {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can manage invitations"));
    }

    let invitations = sqlx::query_as::<_, InvitationRow>(
        "SELECT id, email, role, expires_at, created_at
         FROM invitations
         WHERE workspace_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > NOW()
         ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not load invitations"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not finish invitation lookup"))?;
    Ok(Json(invitations))
}

async fn create_invitation(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(input): Json<CreateInvitationInput>,
) -> ApiResult<(StatusCode, Json<CreatedInvitation>)> {
    let email = input.email.trim().to_lowercase();
    if !is_valid_email(&email) {
        return Err(error(StatusCode::BAD_REQUEST, "Enter a valid email address"));
    }
    let role = input.role.as_deref().unwrap_or("member");
    if role != "member" && role != "admin" {
        return Err(error(StatusCode::BAD_REQUEST, "Invitation role must be member or admin"));
    }

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin invitation creation"))?;
    let can_manage = can_manage_workspace(&mut tx, user_id, workspace_id)
        .await
        .map_err(|cause| internal_error(cause, "Could not verify invitation permissions"))?;
    if !can_manage {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can invite members"));
    }

    let existing_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM workspace_members wm JOIN users u ON u.id = wm.user_id
            WHERE wm.workspace_id = $1 AND lower(u.email) = $2
         )",
    )
    .bind(workspace_id)
    .bind(&email)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not check existing membership"))?;
    if existing_member {
        return Err(error(StatusCode::CONFLICT, "This person is already a workspace member"));
    }

    // Reissuing an invite invalidates every previous link for this email.
    sqlx::query(
        "UPDATE invitations SET revoked_at = NOW()
         WHERE workspace_id = $1 AND lower(email) = $2
           AND accepted_at IS NULL AND revoked_at IS NULL",
    )
    .bind(workspace_id)
    .bind(&email)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not replace a pending invitation"))?;

    let token = make_token();
    let token_hash = hash_token(&token);
    let invitation = sqlx::query_as::<_, InvitationRow>(
        "INSERT INTO invitations (workspace_id, email, role, token_hash, invited_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, NOW() + INTERVAL '7 days')
         RETURNING id, email, role, expires_at, created_at",
    )
    .bind(workspace_id)
    .bind(&email)
    .bind(role)
    .bind(token_hash)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not create invitation"))?;

    let invitee_user_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE lower(email)=$1")
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not find an existing invited account"))?;
    if let Some(invitee_user_id) = invitee_user_id {
        crate::roadmap::create_notification_in_transaction(
            &mut tx,
            workspace_id,
            invitee_user_id,
            Some(user_id),
            "workspace_invitation",
            "Workspace invitation",
            "You have been invited to join a CampusForge workspace.",
            Some("invitation"),
            Some(invitation.id),
            Some(&format!("workspace-invitation:{}", invitation.id)),
        )
        .await
        .map_err(|cause| internal_error(cause, "Could not create invitation notification"))?;
    }

    add_activity(&mut tx, workspace_id, user_id, "invitation_created", format!("invited {email} to the workspace"))
        .await
        .map_err(|cause| internal_error(cause, "Could not record invitation activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save invitation"))?;
    state.publish_workspace_event(workspace_id, None, "invitation_created", format!("invited {email} to the workspace"));

    Ok((StatusCode::CREATED, Json(CreatedInvitation {
        id: invitation.id,
        email: invitation.email,
        role: invitation.role,
        expires_at: invitation.expires_at,
        token,
    })))
}

async fn revoke_invitation(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(invitation_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin invitation revocation"))?;
    let can_manage = can_manage_workspace(&mut tx, user_id, workspace_id)
        .await
        .map_err(|cause| internal_error(cause, "Could not verify invitation permissions"))?;
    if !can_manage {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can manage invitations"));
    }

    let result = sqlx::query(
        "UPDATE invitations SET revoked_at = NOW()
         WHERE id = $1 AND workspace_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL",
    )
    .bind(invitation_id)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not revoke invitation"))?;
    if result.rows_affected() == 0 {
        return Err(error(StatusCode::NOT_FOUND, "Pending invitation not found"));
    }

    sqlx::query("DELETE FROM notifications WHERE workspace_id=$1 AND resource_type='invitation' AND resource_id=$2")
        .bind(workspace_id)
        .bind(invitation_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not remove revoked invitation notifications"))?;

    add_activity(&mut tx, workspace_id, user_id, "invitation_revoked", "revoked a workspace invitation".to_string())
        .await
        .map_err(|cause| internal_error(cause, "Could not record invitation revocation"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save invitation revocation"))?;
    state.publish_workspace_event(workspace_id, None, "invitation_revoked", "revoked a workspace invitation");
    Ok(StatusCode::NO_CONTENT)
}

fn is_assignable_workspace_role(role: &str) -> bool {
    matches!(role, "member" | "admin")
}

#[derive(Debug, Deserialize)]
struct UpdateMemberRoleInput {
    role: String,
}

#[derive(Debug, Serialize)]
struct UpdatedMemberRole {
    user_id: Uuid,
    role: String,
}

async fn update_member_role(
    State(state): State<AppState>,
    WorkspaceUser(actor_id, workspace_id): WorkspaceUser,
    Path(member_id): Path<Uuid>,
    Json(input): Json<UpdateMemberRoleInput>,
) -> ApiResult<Json<UpdatedMemberRole>> {
    let next_role = input.role.trim();
    if !is_assignable_workspace_role(next_role) {
        return Err(error(StatusCode::BAD_REQUEST, "Workspace role must be member or admin"));
    }
    if actor_id == member_id {
        return Err(error(StatusCode::FORBIDDEN, "You cannot change your own workspace role"));
    }

    let mut tx = state.db.begin().await
        .map_err(|cause| internal_error(cause, "Could not begin workspace role update"))?;
    let actor_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM workspace_members WHERE workspace_id=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(actor_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not verify workspace role permissions"))?;
    if actor_role.as_deref() != Some("owner") {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners can change teammate roles"));
    }

    let previous_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM workspace_members WHERE workspace_id=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(member_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not find workspace member"))?;
    let Some(previous_role) = previous_role else {
        return Err(error(StatusCode::NOT_FOUND, "Workspace member not found"));
    };
    if previous_role == "owner" {
        return Err(error(StatusCode::FORBIDDEN, "The workspace owner role cannot be changed"));
    }

    if previous_role == next_role {
        tx.commit().await
            .map_err(|cause| internal_error(cause, "Could not finish workspace role update"))?;
        return Ok(Json(UpdatedMemberRole { user_id: member_id, role: next_role.to_string() }));
    }

    let member_name: String = sqlx::query_scalar("SELECT full_name FROM users WHERE id=$1")
        .bind(member_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not load workspace member profile"))?;

    sqlx::query(
        "UPDATE workspace_members SET role=$3 WHERE workspace_id=$1 AND user_id=$2",
    )
    .bind(workspace_id)
    .bind(member_id)
    .bind(next_role)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not update workspace member role"))?;

    sqlx::query(
        "UPDATE board_members bm SET role=$3
         FROM boards b
         WHERE bm.board_id=b.id AND b.workspace_id=$1 AND bm.user_id=$2",
    )
    .bind(workspace_id)
    .bind(member_id)
    .bind(if next_role == "admin" { "admin" } else { "member" })
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not sync project member roles"))?;

    add_activity(
        &mut tx,
        workspace_id,
        actor_id,
        "member_role_updated",
        format!("changed {member_name}'s role from {previous_role} to {next_role}"),
    )
    .await
    .map_err(|cause| internal_error(cause, "Could not record workspace role change"))?;

    tx.commit().await
        .map_err(|cause| internal_error(cause, "Could not save workspace role change"))?;
    state.publish_workspace_event(
        workspace_id,
        None,
        "member_role_updated",
        format!("changed {member_name}'s role to {next_role}"),
    );

    Ok(Json(UpdatedMemberRole { user_id: member_id, role: next_role.to_string() }))
}

#[cfg(test)]
mod role_tests {
    use super::*;

    #[test]
    fn only_member_and_admin_are_assignable_workspace_roles() {
        assert!(is_assignable_workspace_role("member"));
        assert!(is_assignable_workspace_role("admin"));
        assert!(!is_assignable_workspace_role("owner"));
        assert!(!is_assignable_workspace_role("guest"));
        assert!(!is_assignable_workspace_role(""));
    }
}

#[derive(Debug, Deserialize)]
struct AcceptInvitationInput {
    token: String,
}

#[derive(Debug, Serialize)]
struct AcceptedInvitation {
    workspace_id: Uuid,
}

async fn accept_invitation(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<AcceptInvitationInput>,
) -> ApiResult<Json<AcceptedInvitation>> {
    let token = input.token.trim();
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error(StatusCode::BAD_REQUEST, "Invitation link is invalid"));
    }

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin invitation acceptance"))?;
    let invitation: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, workspace_id::text, email
         FROM invitations
         WHERE token_hash = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > NOW()
         FOR UPDATE",
    )
    .bind(hash_token(token))
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not find invitation"))?;
    let Some((invitation_id, workspace_id_text, invited_email)) = invitation else {
        return Err(error(StatusCode::NOT_FOUND, "Invitation is invalid, expired, or already used"));
    };
    let workspace_id = Uuid::parse_str(&workspace_id_text)
        .map_err(|cause| internal_error(cause, "Invitation workspace ID is invalid"))?;

    let user_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not verify account email"))?
        .ok_or(error(StatusCode::UNAUTHORIZED, "Signed-in account not found"))?;
    if !user_email.eq_ignore_ascii_case(&invited_email) {
        return Err(error(StatusCode::FORBIDDEN, "Sign in using the email address this invitation was sent to"));
    }

    let invitation_role: String = sqlx::query_scalar("SELECT role FROM invitations WHERE id = $1")
        .bind(invitation_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not load invitation role"))?;

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role)
         VALUES ($1, $2, $3) ON CONFLICT (workspace_id, user_id) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(invitation_role)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not add workspace member"))?;

    sqlx::query(
        "INSERT INTO board_members (board_id, user_id, role)
         SELECT b.id, wm.user_id,
                CASE WHEN wm.role IN ('owner', 'admin') THEN 'admin' ELSE 'member' END
         FROM boards b
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = $1
         WHERE b.workspace_id = $2
         ON CONFLICT (board_id, user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not add project memberships"))?;

    sqlx::query("UPDATE invitations SET accepted_at = NOW() WHERE id = $1")
        .bind(invitation_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not mark invitation accepted"))?;

    sqlx::query(
        "INSERT INTO activity_events (workspace_id, actor_id, event_type, payload)
         VALUES ($1, $2, 'member_joined', jsonb_build_object('description', 'joined the workspace'))",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not record new member activity"))?;

    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save workspace membership"))?;
    state.publish_workspace_event(workspace_id, None, "member_joined", "A new member joined the workspace");
    Ok(Json(AcceptedInvitation { workspace_id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_tokens_are_random_hex_and_only_the_hash_is_stored() {
        let first = make_token();
        let second = make_token();
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
        assert_eq!(hash_token(&first).len(), 64);
        assert_ne!(hash_token(&first), first);
        assert_eq!(hash_token(&first), hash_token(&first));
    }

    #[test]
    fn invitation_email_validation_rejects_malformed_addresses() {
        assert!(is_valid_email("student@example.edu"));
        assert!(is_valid_email("student+team@example.co.in"));
        assert!(!is_valid_email("not-an-email"));
        assert!(!is_valid_email("student@localhost"));
        assert!(!is_valid_email("student @example.edu"));
        assert!(!is_valid_email("@example.edu"));
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_invitations).post(create_invitation))
        .route("/accept", post(accept_invitation))
        .route("/{invitation_id}", delete(revoke_invitation))
        .route("/members/{member_id}/role", patch(update_member_role))
}
