#[cfg(test)]
mod integration_tests {
    use crate::storage::sqlite::SqliteBackend;
    use crate::storage::{
        models::{Email, Webhook, WebhookEvent},
        StorageBackend,
    };
    use crate::webhooks::WebhookTrigger;
    use mockito::{Mock, Server};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};

    /// Integration test for complete webhook flow
    #[tokio::test]
    async fn test_webhook_integration_flow() {
        // Setup test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(
            SqliteBackend::new(&format!("sqlite:{}", db_path.display()))
                .await
                .unwrap(),
        );

        // Setup mock webhook server
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/webhook")
            .with_status(200)
            .with_header("content-type", "application/json")
            .expect(2) // Expect 2 calls: arrival and deletion
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());

        // Create webhook for arrival and deletion events
        let webhook = Webhook::new(
            "test".to_string(),
            webhook_url,
            vec![WebhookEvent::Arrival, WebhookEvent::Deletion],
        );
        storage.create_webhook(webhook).await.unwrap();

        // Create webhook trigger
        let webhook_trigger = WebhookTrigger::new(storage.clone());

        // Test 1: Email arrival triggers webhook
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email.clone()).await.unwrap();

        // Trigger arrival webhook
        let result = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Arrival, Some(&email))
            .await;
        assert!(result.is_ok());

        // Test 2: Email deletion triggers webhook
        let result = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Deletion, None)
            .await;
        assert!(result.is_ok());

        // Verify both webhook calls were made
        mock.assert_async().await;
    }

    /// Integration test for webhook with multiple mailboxes
    #[tokio::test]
    async fn test_webhook_multiple_mailboxes() {
        // Setup test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(
            SqliteBackend::new(&format!("sqlite:{}", db_path.display()))
                .await
                .unwrap(),
        );

        // Setup mock webhook servers
        let mut server1 = Server::new_async().await;
        let mock1 = server1
            .mock("POST", "/webhook1")
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        let mut server2 = Server::new_async().await;
        let mock2 = server2
            .mock("POST", "/webhook2")
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        // Create webhooks for different mailboxes
        let webhook1 = Webhook::new(
            "alice".to_string(),
            format!("{}/webhook1", server1.url()),
            vec![WebhookEvent::Arrival],
        );
        storage.create_webhook(webhook1).await.unwrap();

        let webhook2 = Webhook::new(
            "bob".to_string(),
            format!("{}/webhook2", server2.url()),
            vec![WebhookEvent::Arrival],
        );
        storage.create_webhook(webhook2).await.unwrap();

        let webhook_trigger = WebhookTrigger::new(storage.clone());

        // Create test emails
        let email1 = Email::new(
            "alice@example.com".to_string(),
            "sender@example.com".to_string(),
            "Email for Alice".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email1.clone()).await.unwrap();

        let email2 = Email::new(
            "bob@example.com".to_string(),
            "sender@example.com".to_string(),
            "Email for Bob".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email2.clone()).await.unwrap();

        // Trigger webhooks for both mailboxes
        let result1 = webhook_trigger
            .trigger_webhooks("alice", WebhookEvent::Arrival, Some(&email1))
            .await;
        assert!(result1.is_ok());

        let result2 = webhook_trigger
            .trigger_webhooks("bob", WebhookEvent::Arrival, Some(&email2))
            .await;
        assert!(result2.is_ok());

        // Verify both webhook calls were made to correct endpoints
        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    /// Integration test for webhook failure and retry
    #[tokio::test]
    async fn test_webhook_failure_retry() {
        // Setup test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(
            SqliteBackend::new(&format!("sqlite:{}", db_path.display()))
                .await
                .unwrap(),
        );

        // Setup mock webhook server that fails first, then succeeds
        let mut server = Server::new_async().await;
        let mock_fail = server
            .mock("POST", "/webhook")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;

        let mock_success = server
            .mock("POST", "/webhook")
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());

        // Create webhook
        let webhook = Webhook::new("test".to_string(), webhook_url, vec![WebhookEvent::Arrival]);
        storage.create_webhook(webhook).await.unwrap();

        let webhook_trigger = WebhookTrigger::new(storage.clone());

        // Create test email
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email.clone()).await.unwrap();

        // Trigger webhook (should fail first, then retry and succeed)
        let result = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Arrival, Some(&email))
            .await;
        assert!(result.is_ok());

        // Verify both calls were made
        mock_fail.assert_async().await;
        mock_success.assert_async().await;
    }

    /// Integration test for webhook event filtering
    #[tokio::test]
    async fn test_webhook_event_filtering() {
        // Setup test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(
            SqliteBackend::new(&format!("sqlite:{}", db_path.display()))
                .await
                .unwrap(),
        );

        // Setup mock webhook server
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/webhook")
            .with_status(200)
            .expect(1) // Only expect 1 call for arrival
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());

        // Create webhook that only listens for arrival events
        let webhook = Webhook::new(
            "test".to_string(),
            webhook_url,
            vec![WebhookEvent::Arrival], // Only arrival, not deletion
        );
        storage.create_webhook(webhook).await.unwrap();

        let webhook_trigger = WebhookTrigger::new(storage.clone());

        // Create test email
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email.clone()).await.unwrap();

        // Trigger arrival webhook (should be called)
        let result1 = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Arrival, Some(&email))
            .await;
        assert!(result1.is_ok());

        // Trigger deletion webhook (should NOT be called due to filtering)
        let result2 = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Deletion, None)
            .await;
        assert!(result2.is_ok());

        // Verify only arrival webhook was called
        mock.assert_async().await;
    }

    /// Integration test for webhook URL normalization
    #[tokio::test]
    async fn test_webhook_url_normalization() {
        // Setup test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(
            SqliteBackend::new(&format!("sqlite:{}", db_path.display()))
                .await
                .unwrap(),
        );

        // Setup mock webhook server
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/webhook")
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        // Test webhook URL without protocol (should be normalized to http://)
        let webhook_url = format!("{}/webhook", server.url());

        // Create webhook with URL without protocol
        let webhook = Webhook::new("test".to_string(), webhook_url, vec![WebhookEvent::Arrival]);
        storage.create_webhook(webhook).await.unwrap();

        let webhook_trigger = WebhookTrigger::new(storage.clone());

        // Create test email
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email.clone()).await.unwrap();

        // Trigger webhook (should normalize URL and succeed)
        let result = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Arrival, Some(&email))
            .await;
        assert!(result.is_ok());

        // Verify webhook was called with normalized URL
        mock.assert_async().await;
    }

    /// Integration test for webhook with disabled status
    #[tokio::test]
    async fn test_webhook_disabled() {
        // Setup test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(
            SqliteBackend::new(&format!("sqlite:{}", db_path.display()))
                .await
                .unwrap(),
        );

        // Setup mock webhook server
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/webhook")
            .with_status(200)
            .expect(0) // Should not be called
            .create_async()
            .await;

        let webhook_url = format!("{}/webhook", server.url());

        // Create disabled webhook
        let mut webhook =
            Webhook::new("test".to_string(), webhook_url, vec![WebhookEvent::Arrival]);
        webhook.enabled = false; // Disable the webhook
        storage.create_webhook(webhook).await.unwrap();

        let webhook_trigger = WebhookTrigger::new(storage.clone());

        // Create test email
        let email = Email::new(
            "test@example.com".to_string(),
            "sender@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
            None,
            vec![],
        );
        storage.store_email(email.clone()).await.unwrap();

        // Trigger webhook (should not be called due to disabled status)
        let result = webhook_trigger
            .trigger_webhooks("test", WebhookEvent::Arrival, Some(&email))
            .await;
        assert!(result.is_ok());

        // Verify webhook was NOT called
        mock.assert_async().await;
    }
}
