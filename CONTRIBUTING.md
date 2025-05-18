# Contributing to MailSender

Thank you for considering contributing to MailSender! This document outlines the process for contributing to the project and how to set up your local development environment.

## Local Development Setup

### Prerequisites

- Rust (stable channel) - [Installation guide](https://www.rust-lang.org/tools/install)
- PostgreSQL - [Installation guide](https://www.postgresql.org/download/)
- Git

### Setup Steps

1. **Fork and Clone the Repository**

   ```bash
   git clone https://github.com/yourusername/mailsender.git
   cd mailsender
   ```

2. **Set Up PostgreSQL Database**

   Create a PostgreSQL database for development:

   ```bash
   createdb mailsender_dev
   ```

   You can also use a containerized PostgreSQL instance:

   ```bash
   docker run --name postgres-mailsender -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=mailsender_dev -p 5432:5432 -d postgres:14
   ```

3. **Configure Environment Variables**

   Create a `.env` file in the project root:

   ```bash
   cp .env.example .env
   ```

   Modify the `.env` file with your database connection details:

   ```
   DATABASE_URL=postgres://postgres:postgres@localhost:5432/mailsender_dev
   PGHOST=localhost
   PGUSER=postgres
   PGPASSWORD=postgres
   PGDATABASE=mailsender_dev
   PGPORT=5432
   CONFIG_PATH=config.yaml
   RUST_LOG=debug
   ```

4. **Configure the Application**

   Create a configuration file:

   ```bash
   cp config.example.yaml config.yaml
   ```

   Modify the YAML configuration to suit your development needs.

5. **Install Cargo Tools (Optional but Recommended)**

   ```bash
   # For better error messages
   cargo install cargo-watch

   # For profiling performance
   cargo install cargo-flamegraph
   ```

## Development Workflow

### Running the Application Locally

1. **Build the project:**

   ```bash
   cargo build
   ```

2. **Run migrations:**

   The migrations will run automatically when the application starts, but you can also run:

   ```bash
   cargo run -- db migrate
   ```

3. **Run the application:**

   ```bash
   cargo run
   ```

   For development with automatic reloading:

   ```bash
   cargo watch -x run
   ```

### Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run specific tests
cargo test email_sending

# Run with verbose output
cargo test -- --nocapture
```

### Debugging

For detailed logging, set the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run
```

## Pull Request Process

1. **Create a Branch:**

   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make Your Changes:**
   - Follow the code style of the project
   - Write tests for your changes when applicable
   - Keep your changes focused and limited to the feature/bugfix at hand

3. **Commit Your Changes:**

   ```bash
   git commit -m "Add meaningful commit message"
   ```

4. **Push to Your Fork:**

   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open a Pull Request:**
   - Go to the GitHub repository and open a pull request
   - Clearly describe the changes and link to any related issues
   - Ensure the PR description follows the template

6. **Code Review:**
   - Maintainers will review your PR
   - Address any review comments and update your PR

## Development Guidelines

### Code Style

This project follows Rust's standard style guidelines:

- Run `cargo fmt` before committing to ensure consistent formatting
- Use `cargo clippy` to catch common mistakes and improve code quality

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb in the present tense (e.g., "Add", "Fix", "Update")
- Reference issue numbers in the commit message when applicable

### Documentation

- Add comments to explain complex code
- Update the README.md if you change functionality
- Document public API functions and types

### Testing

- Write unit tests for new functionality
- Ensure all tests pass before submitting a PR
- Consider edge cases in your tests

## Performance Profiling

For performance-critical contributions, you can use flamegraphs to identify bottlenecks:

```bash
# Generate a flamegraph
cargo flamegraph

# Run with a specific workload
cargo flamegraph --bin mailsender -- --some-arguments
```

## Security Considerations

- Never commit secrets or credentials
- Use the environment variables for sensitive information
- Report security vulnerabilities directly to maintainers, not as public issues