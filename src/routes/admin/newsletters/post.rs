use crate::authentication::UserId;
use crate::idempotency::{save_response, try_processing, IdempotencyKey, NextAction};
use crate::utils::{e400, e500, see_other};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name="Publish a newsletter issue"
    skip(form, pool, user_id)
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key = IdempotencyKey::try_from(idempotency_key).map_err(e400)?;

    let mut transaction = match try_processing(&pool, &idempotency_key, **user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    };

    if let Err(e) = validate_newsletter(&title, &html_content, &text_content) {
        FlashMessage::error(e.to_string()).send();
        return Ok(see_other("/admin/newsletters"));
    };

    let newsletter_issue_id =
        insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
            .await
            .map_err(e500)?;

    enqueue_delivery_tasks(&mut transaction, newsletter_issue_id)
        .await
        .map_err(e500)?;

    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, **user_id, response)
        .await
        .map_err(e500)?;

    success_message().send();
    Ok(response)
}

fn validate_newsletter(title: &str, html: &str, text: &str) -> Result<(), anyhow::Error> {
    if title.is_empty() {
        Err(anyhow::anyhow!("Title must not be empty"))
    } else if html.is_empty() {
        Err(anyhow::anyhow!("HTML content must not be empty"))
    } else if text.is_empty() {
        Err(anyhow::anyhow!("Text content must not be empty"))
    } else {
        Ok(())
    }
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'static, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, anyhow::Error> {
    let newsletter_issue_id = uuid::Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO newsletters_issues (
            newsletter_issue_id,
            title,
            text_content, 
            html_content,
            published_at
        ) VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    );
    transaction.execute(query).await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'static, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email,
            n_retries
        )
        SELECT $1, email, 0
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id
    );
    transaction.execute(query).await?;
    Ok(())
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}
