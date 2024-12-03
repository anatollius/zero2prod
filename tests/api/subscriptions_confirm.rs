use crate::helpers::spawn_app;

#[tokio::test]
async fn confirmations_without_tokens_are_rejected_with_a_400() {
    // Arraane
    let app = spawn_app().await;

    // Act
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    // Assert
    assert_eq!(400, response.status().as_u16())
}
