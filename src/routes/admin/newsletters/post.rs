use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::{get_saved_response, save_response, IdempotencyKey};
use crate::utils::{e400, e500, see_other};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name="Publish a newsletter issue"
    skip(form, pool, email_client, user_id)
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key = IdempotencyKey::try_from(idempotency_key).map_err(e400)?;

    if let Some(saved_response) = get_saved_response(&pool, &idempotency_key, **user_id)
        .await
        .map_err(e500)?
    {
        FlashMessage::info("The newsletter issue has been published!").send();
        return Ok(saved_response);
    }

    match validate_newsletter(&title, &html_content, &text_content) {
        Err(e) => {
            FlashMessage::error(e.to_string()).send();
            return Ok(see_other("/admin/newsletters"));
        }
        _ => {}
    };

    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;

    for subscriber in subscribers {
        match subscriber {
            Ok(email) => email_client
                .send_email(&email, &title, &html_content, &text_content)
                .await
                .with_context(|| format!("Failed to send newsletter issue to {}", &email))
                .map_err(e500)?,
            Err(error) => tracing::warn!(
                error.cause_chain = ?error,
                "Skipping a confirmed subscriber. Their stored contact details are invalid.",
            ),
        }
    }

    FlashMessage::info("The newsletter issue has been published!").send();
    let response = see_other("/admin/newsletters");
    let response = save_response(&pool, &idempotency_key, **user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
}

fn validate_newsletter(title: &str, html: &str, text: &str) -> Result<(), anyhow::Error> {
    if title.len() == 0 {
        Err(anyhow::anyhow!("Title must not be empty"))
    } else if html.len() == 0 {
        Err(anyhow::anyhow!("HTML content must not be empty"))
    } else if text.len() == 0 {
        Err(anyhow::anyhow!("Text content must not be empty"))
    } else {
        Ok(())
    }
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<SubscriberEmail, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!("SELECT email FROM subscriptions WHERE status = 'confirmed';")
        .fetch_all(pool)
        .await
        .context("Failed to get subscribers from the database.")?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(email),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}
