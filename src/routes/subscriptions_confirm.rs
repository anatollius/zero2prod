use crate::routes::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(
    pool: web::Data<PgPool>,
    parameters: web::Query<Parameters>,
) -> Result<HttpResponse, ConfirmSubscriberError> {
    if let Some(subscriber_id) = get_subscriber_id_from_token(&pool, &parameters.subscription_token)
        .await
        .context("Failed to query the subscription tokens table to get the subscriber_id")?
    {
        confirm_subscriber(&pool, subscriber_id)
            .await
            .context("Failed to update subscriptions table to confirm subscriber")?;
    } else {
        return Err(ConfirmSubscriberError::UnauthorizedError());
    }

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(pool, subscription_token))]
async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT subscriber_id from subscription_tokens
        WHERE subscription_token = $1
        "#,
        subscription_token,
    )
    .fetch_optional(pool)
    .await?;
    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(pool, subscriber_id))]
async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions SET status = 'confirmed' WHERE id = $1
        "#,
        subscriber_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(thiserror::Error)]
pub enum ConfirmSubscriberError {
    #[error("Unauthorized")]
    UnauthorizedError(),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConfirmSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl actix_web::ResponseError for ConfirmSubscriberError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::UnauthorizedError() => StatusCode::UNAUTHORIZED,
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
