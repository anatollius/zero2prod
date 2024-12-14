use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_change_password_form() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let response = app.get_change_password().await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_change_your_password() {
    // Arrange
    let app = spawn_app().await;
    let new_password = "top_secret";

    // Act
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": "also_top_secret",
            "new_password": new_password,
            "new_password_check": new_password,
        }))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}
