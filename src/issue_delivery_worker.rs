use crate::configuration::Settings;
use crate::startup::get_connection_pool;
use crate::{domain::SubscriberEmail, email_client::EmailClient};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::time::Duration;
use tracing::{field::display, Span};
use uuid::Uuid;

pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);
    let email_client = configuration.email_client.client();

    worker_loop(connection_pool, email_client).await
}

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty
    ),
    err
)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    if let Some((transaction, issue_id, email, n_retries)) = dequeue_task(pool).await? {
        Span::current()
            .record("newsletter_issue_id", display(&issue_id))
            .record("subscriber_email", display(&email));

        match SubscriberEmail::parse(email.clone()) {
            Ok(email) => {
                let issue = get_issue(pool, issue_id).await?;
                if let Err(e) = email_client
                    .send_email(
                        &email,
                        &issue.title,
                        &issue.html_content,
                        &issue.text_content,
                    )
                    .await
                {
                    if n_retries < 5 {
                        increment_retries(transaction, issue_id, email.to_string(), n_retries)
                            .await?;

                        return Ok(ExecutionOutcome::TaskCompleted);
                    } else {
                        tracing::error!(
                            error.cause_chain = ?e,
                            "Failed to deliver issue to a confirmed subscriber. Skipping.",
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid."
                )
            }
        }

        delete_task(transaction, issue_id, email).await?;
        Ok(ExecutionOutcome::TaskCompleted)
    } else {
        Ok(ExecutionOutcome::EmptyQueue)
    }
}

type PgTransaction = Transaction<'static, Postgres>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String, i16)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let r = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email, n_retries
        FROM issue_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some(r) = r {
        Ok(Some((
            transaction,
            r.newsletter_issue_id,
            r.subscriber_email,
            r.n_retries,
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(
    "Deleting task from issue delivery queue"
    skip(transaction))
]
async fn delete_task(
    mut transaction: PgTransaction,
    newsletter_issue_id: Uuid,
    subscriber_email: String,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE newsletter_issue_id = $1
        AND subscriber_email = $2
        "#,
        newsletter_issue_id,
        subscriber_email
    );
    transaction.execute(query).await?;
    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content
        FROM newsletters_issues
        WHERE newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;

    Ok(issue)
}

#[tracing::instrument(
    "Incrementing retry"
    skip(transaction)
)]
async fn increment_retries(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    subscriber_email: String,
    n_retries: i16,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
        UPDATE issue_delivery_queue
        SET n_retries = $1
        WHERE newsletter_issue_id = $2
        AND subscriber_email = $3
        "#,
        n_retries + 1,
        issue_id,
        subscriber_email
    );
    transaction.execute(query).await?;
    transaction.commit().await?;
    Ok(())
}

async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}
