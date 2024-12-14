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

#[tokio::test]
async fn new_password_fields_must_match() {
    // Arrange
    let app = spawn_app().await;

    // Act - Part 1 - Log in
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // Act - Part 2 - Try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": "new_password",
            "new_password_check": "different_new_password"
        }))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains(
        "<p><i>You entered two different new passwords - \
        the field values must match.</i></p>"
    ))
}

#[tokio::test]
async fn current_password_must_be_valid() {
    // Arrange
    let app = spawn_app().await;

    // Act - Part 1 - Log in
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // Act - Part 2 - Try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": "incorrect_password",
            "new_password": "new_password",
            "new_password_check": "new_password"
        }))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>The current password is incorrect.</i></p>"))
}

#[tokio::test]
async fn the_new_password_is_too_short() {
    // Arrange
    // Arrange
    let app = spawn_app().await;

    // Act - Part 1 - Log in
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // Act - Part 2 - Try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": "short",
            "new_password_check": "short"
        }))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains(
        "<p><i>The new password is too short - \
        it must be at least 12 characters long.</i></p>"
    ))
}

#[tokio::test]
async fn change_password_successfully() {
    // Arrange
    let app = spawn_app().await;
    let new_password = "a_great_password";

    // Act - Part 1 - Log in
    let response = app
        .post_login(&serde_json::json!({
            "username": app.test_user.username,
            "password": app.test_user.password
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    // Act - Part 2 - Change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": new_password,
            "new_password_check": new_password
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>Your password has been changed</i></p>"));

    // Act - Part 4 - Log out
    let response = app.post_logout().await;
    assert_is_redirect_to(&response, "/login");

    // Act - Part 5 - Log in using the new password
    let response = app
        .post_login(&serde_json::json!({
            "username": app.test_user.username,
            "password": new_password
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard");
}
