# MailSender

A lightweight, Rust-based, headless SMTP send-only service with PostgreSQL storage, designed for high deliverability and easy deployment.

## Features

- Headless SMTP send-only service built in Rust
- PostgreSQL integration for email data and key storage
- Support for multiple domains and email addresses
- Automatic key generation and DNS configuration output
- Email authentication (SPF, DKIM, DMARC)
- Simple YAML configuration
- High deliverability focus
- Email queuing and retry mechanism
- Rate limiting
- REST API for sending emails

## Getting Started

### Prerequisites

- PostgreSQL database
- Rust (stable channel)
- SMTP server for outgoing emails (optional, you can use an external service)

### Installation

1. Clone this repository:
   ```
   git clone https://github.com/yourusername/mailsender.git
   cd mailsender
   