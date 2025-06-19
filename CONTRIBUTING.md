# Contributing to cqrs-rust-lib

Thank you for considering contributing to cqrs-rust-lib! This document provides guidelines and instructions for contributing to this project.

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for everyone.

## Getting Started

### Prerequisites

- Rust (stable channel)
- Cargo
- MongoDB (for running tests with MongoDB features)

### Setting Up the Development Environment

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```
   git clone https://github.com/YOUR-USERNAME/cqrs-rust-lib.git
   cd cqrs-rust-lib
   ```
3. Add the original repository as an upstream remote:
   ```
   git remote add upstream https://github.com/ORIGINAL-OWNER/cqrs-rust-lib.git
   ```
4. Install dependencies:
   ```
   cargo build
   ```

## Development Workflow

1. Create a new branch for your feature or bugfix:
   ```
   git checkout -b feature/your-feature-name
   ```
   or
   ```
   git checkout -b fix/issue-description
   ```

2. Make your changes, following the coding standards below

3. Run tests to ensure your changes don't break existing functionality:
   ```
   cargo test
   ```
   
   For MongoDB tests:
   ```
   MONGODB_TEST_URI=mongodb://localhost:27017 cargo test --features mongodb
   ```

4. Commit your changes with a descriptive commit message:
   ```
   git commit -m "Add feature: description of your changes"
   ```

5. Push your branch to your fork:
   ```
   git push origin feature/your-feature-name
   ```

6. Open a pull request against the main repository

## Coding Standards

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format your code
- Run `cargo clippy` to catch common mistakes and improve your code
- Write tests for new functionality
- Update documentation for public API changes

## Pull Request Process

1. Ensure your code passes all tests and CI checks
2. Update the README.md with details of changes to the interface, if applicable
3. Update the documentation if you're changing public APIs
4. The PR should work with all feature combinations
5. Your PR will be reviewed by maintainers, who may request changes

## Reporting Issues

When reporting issues, please include:

- A clear and descriptive title
- A detailed description of the issue
- Steps to reproduce the behavior
- Expected behavior
- Actual behavior
- Environment details (OS, Rust version, etc.)
- Any relevant logs or error messages

## Feature Requests

Feature requests are welcome! Please provide:

- A clear and detailed description of the feature
- The motivation for the feature
- Possible implementation approaches, if you have ideas

## License

By contributing to this project, you agree that your contributions will be licensed under the project's license.