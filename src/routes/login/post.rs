use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use crate::startup::HmacSecret;
use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{HttpResponse, ResponseError};
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use sqlx::PgPool;

#[tracing::instrument(
    skip(credentials, pool,secret),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    credentials: web::Form<Credentials>,
    pool: web::Data<PgPool>,
    secret: web::Data<HmacSecret>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    tracing::Span::current().record("username", tracing::field::display(&credentials.0.username));

    match validate_credentials(credentials.0, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));

            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish())
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };
            let query_string = format!("error={}", urlencoding::Encoded::new(e.to_string()));

            let hmac_tag = {
                let mut mac =
                    Hmac::<sha2::Sha256>::new_from_slice(secret.0.expose_secret().as_bytes())
                        .unwrap();
                mac.update(query_string.as_bytes());
                mac.finalize().into_bytes()
            };
            let response = HttpResponse::build(e.status_code())
                .insert_header((LOCATION, format!("/login?{query_string}&tag={hmac_tag:x}")))
                .finish();

            Err(InternalError::from_response(e, response))
        }
    }
    // .await
    // .map_err(|e| match e {
    //     AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
    //     AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
    // })?;
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[source] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LoginError {
    // fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
    //     let query_string = format!("error={}", urlencoding::Encoded::new(self.to_string()));
    //     let secret: &[u8] = todo!();
    //     let hmac_tag = {
    //         let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).unwrap();
    //         mac.update(query_string.as_bytes());
    //         mac.finalize().into_bytes()
    //     };
    //     HttpResponse::build(self.status_code())
    //         .insert_header((LOCATION, format!("/login?{query_string}&tag={hmac_tag:x}")))
    //         .finish()
    // }
    fn status_code(&self) -> StatusCode {
        StatusCode::SEE_OTHER
    }
}
