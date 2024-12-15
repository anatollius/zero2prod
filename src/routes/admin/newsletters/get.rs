use crate::authentication::UserId;
use actix_web::http::header::ContentType;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn publish_newsletter_form(
    flash: IncomingFlashMessages,
    _user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let mut msg_html = String::new();
    for msg in flash.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", msg.content()).unwrap()
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Newsletters</title>
</head>
<body>
    <p>Issue a new newsletter</p>
    {msg_html}
    <form action="/admin/newsletters" method="post">
        <label>Title:<br>
            <input
                type="text"
                placeholder="Enter the issue title"
                name="title"
            >
        </label>
        <br>
        <label>Plain text content:<br>
            <textarea
                placeholder="Enter the content in plain text"
                name="text_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <label>HTML content:<br>
            <textarea
                placeholder="Enter the content in HTML format"
                name="html_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <button type="submit">Publish</button>
    </form>
</body>
</html>"#,
        )))
}
