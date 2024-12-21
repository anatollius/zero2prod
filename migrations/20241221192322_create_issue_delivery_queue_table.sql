CREATE TABLE issue_delivery_queue (
    newsletter_issue_id uuid NOT NULL REFERENCES newsletters_issues (newsletter_issue_id),
    subscriber_email TEXT NOT NULL,
    PRIMARY KEY(newsletter_issue_id, subscriber_email)
)
