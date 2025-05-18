# MailSender

A lightweight, Rust-based, headless SMTP send-only service with PostgreSQL storage, designed for high deliverability and easy deployment. An alternative to services like SendGrid or Mailgun, but self-hosted and privacy-focused.

## Features

- **Lightweight Architecture**: Headless SMTP send-only service built in Rust for performance and minimal resource usage
- **PostgreSQL Integration**: Reliable storage for email data, tracking, and DKIM keys
- **Multi-Domain Support**: Configure and manage multiple domains and email addresses from a single service
- **Automatic Key Management**: Generate and store DKIM keys for strong email authentication
- **DNS Configuration Guidance**: Automatically outputs all necessary DNS entries for proper email setup
- **Email Authentication**: Full SPF, DKIM, and DMARC support for maximum deliverability
- **Rate Limiting**: Configurable limits per minute, hour, and day to maintain good sender reputation
- **Retry Mechanism**: Intelligent retry system with exponential backoff for temporary failures
- **Simple REST API**: Clean and straightforward HTTP API for sending emails
- **Easy Deployment**: Designed for simple deployment with Docker and Coolify

## Quick Start Guide

### Prerequisites

- PostgreSQL database
- Docker (for containerized deployment)
- SMTP relay (optional, can use an external service or your own server)

### Installation Options

#### 1. Docker Deployment (Recommended)

```bash
# Pull the image
docker pull ghcr.io/yourusername/mailsender:latest

# Run with PostgreSQL connection
docker run -d --name mailsender \
  -p 8000:8000 \
  -v /path/to/config.yaml:/app/config.yaml \
  -e DATABASE_URL=postgres://user:password@host:5432/mailsender \
  ghcr.io/yourusername/mailsender:latest
```

#### 2. Coolify Deployment

1. In your Coolify dashboard, go to "Resources" → "Create new resource"
2. Select "Application" and choose "GitHub/GitLab" as the source
3. Connect to your Git provider and select this repository
4. For the build settings:
   - Build command: `cargo build --release`
   - Start command: `./target/release/mailsender`
   - Port: `8000`
5. Add the following environment variables:
   - `DATABASE_URL`: Your PostgreSQL connection string
   - `CONFIG_PATH`: `/app/config.yaml` (or customize)
6. Under "Advanced settings", add a persistent volume:
   - Source: `/path/on/host/config`
   - Destination: `/app/config`
7. Deploy the service
8. After first deployment, check the logs for DNS configuration instructions
9. Set up the required DNS records for your domain

#### 3. From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/mailsender.git
cd mailsender

# Build the project
cargo build --release

# Setup environment variables
cp .env.example .env
# Edit .env with your PostgreSQL connection details

# Create configuration
cp config.example.yaml config.yaml
# Edit config.yaml with your domain settings

# Run the service
cargo run --release
```

### First Run & DNS Configuration

On first run, MailSender will automatically:

1. Set up necessary database tables
2. Generate DKIM keys for your configured domains
3. Output DNS configuration instructions

**Important:** Follow the DNS configuration instructions exactly to ensure high deliverability. You'll need to add TXT records for:
- DKIM (domain keys)
- SPF (sender policy framework)
- DMARC (domain-based message authentication)

## API Usage

### Send an Email

```bash
curl -X POST http://localhost:8000/api/email \
  -H "Content-Type: application/json" \
  -d '{
    "from": "noreply@yourdomain.com",
    "to": "recipient@example.com",
    "subject": "Hello from MailSender",
    "text": "This is a plain text message",
    "html": "<p>This is an <strong>HTML</strong> message</p>"
  }'
```

Response:
```json
{
  "id": "5f8d4e32-1234-5678-abcd-1234567890ab",
  "status": "queued"
}
```

### Check Email Status

```bash
curl http://localhost:8000/api/email/5f8d4e32-1234-5678-abcd-1234567890ab
```

Response:
```json
{
  "id": "5f8d4e32-1234-5678-abcd-1234567890ab",
  "status": "sent",
  "created_at": "2023-01-01T12:00:00Z",
  "updated_at": "2023-01-01T12:00:05Z",
  "sent_at": "2023-01-01T12:00:05Z",
  "retry_count": 0
}
```

## Configuration Reference

### config.yaml

```yaml
domains:
  - domain: example.com
    from_addresses:
      - noreply@example.com
      - support@example.com
      # Use "*" to allow any address from this domain
    dkim_selector: default

smtp:
  hostname: smtp.example.com  # SMTP server to use for sending
  port: 587                   # SMTP port (usually 25, 465, or 587)
  username: ~                 # Optional SMTP username (omit or use ~ for none)
  password: ~                 # Optional SMTP password (omit or use ~ for none)
  max_connections: 5          # Maximum number of SMTP connections

rate_limits:
  emails_per_minute: 60
  emails_per_hour: 1000
  emails_per_day: 10000

retries:
  max_retries: 3              # Maximum number of retry attempts
  initial_backoff_seconds: 60  # Wait 60 seconds before first retry
  max_backoff_seconds: 3600    # Maximum wait of 1 hour between retries
```

## Security Considerations

- Store your SMTP credentials securely using environment variables
- Use strong DKIM keys (automatically generated by MailSender)
- Set appropriate rate limits to prevent abuse
- Use TLS encryption for SMTP connections
- Regularly check your email delivery status and DMARC reports

## Troubleshooting

### Common Issues

#### Email Delivery Issues

- **Problem**: Emails being marked as spam or not being delivered
- **Solution**: Verify your DNS records are properly set up (DKIM, SPF, DMARC). Check the service logs for proper DNS configuration instructions.

#### SMTP Connection Failures

- **Problem**: Service fails to connect to SMTP server
- **Solution**: 
  - Verify SMTP credentials are correct
  - Check if port 587 (or your configured port) is open
  - Ensure TLS is properly configured on your SMTP server

#### Database Connection Issues

- **Problem**: Service fails to connect to PostgreSQL
- **Solution**:
  - Verify your DATABASE_URL is correct
  - Check that PostgreSQL is running and accessible
  - Ensure the database user has proper permissions

## Contributing

Contributions are welcome! Here's how you can help:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please make sure to update tests as appropriate.

## License

MIT License - See LICENSE file for details
   