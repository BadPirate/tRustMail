-- Create email_status enum type
CREATE TYPE email_status AS ENUM ('pending', 'sending', 'sent', 'failed');

-- Create emails table
CREATE TABLE IF NOT EXISTS emails (
    id UUID PRIMARY KEY,
    from_email VARCHAR(255) NOT NULL,
    to_email VARCHAR(255) NOT NULL,
    subject TEXT NOT NULL,
    body_text TEXT NOT NULL,
    body_html TEXT,
    status email_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    sent_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    error_message TEXT
);

-- Create index on status and next_retry_at for efficient queue processing
CREATE INDEX IF NOT EXISTS idx_emails_status_retry ON emails (status, next_retry_at);

-- Create index on from_email and created_at for rate limiting
CREATE INDEX IF NOT EXISTS idx_emails_from_created ON emails (from_email, created_at);

-- Create domain_keys table for DKIM keys
CREATE TABLE IF NOT EXISTS domain_keys (
    id UUID PRIMARY KEY,
    domain VARCHAR(255) NOT NULL,
    selector VARCHAR(50) NOT NULL,
    private_key TEXT NOT NULL,
    public_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (domain, selector)
);

-- Create email_attachments table (for future use)
CREATE TABLE IF NOT EXISTS email_attachments (
    id UUID PRIMARY KEY,
    email_id UUID NOT NULL REFERENCES emails(id),
    filename VARCHAR(255) NOT NULL,
    content_type VARCHAR(100) NOT NULL,
    content BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    size_bytes INTEGER NOT NULL
);
