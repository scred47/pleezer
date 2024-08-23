# Contributing to pleezer

Thank you for your interest in contributing to **pleezer**! Your support is crucial to improving this project. This guide will help you understand how to report bugs, submit pull requests, and engage with our community.

## Table of Contents

- [Code of Conduct and Project Governance](#code-of-conduct-and-project-governance)
- [Getting Started](#getting-started)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Feature Requests](#feature-requests)
  - [Pull Request Process](#pull-request-process)
  - [Security](#security)
- [Project History](#project-history)
- [Financial Contributions](#financial-contributions)
- [CI Pipeline](#ci-pipeline)
- [Automated Testing](#automated-testing)
- [Performance Testing](#performance-testing)
- [Coding Conventions](#coding-conventions)
- [Documentation](#documentation)
- [Release Process](#release-process)
- [Acknowledgments](#acknowledgments)

## Code of Conduct and Project Governance

We require all contributors to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md), which ensures a respectful and constructive environment for everyone. The project is currently maintained by the author, who has the final decision-making authority. We expect all contributors to resolve any conflicts, either technical or interpersonal, in a manner that aligns with the Code of Conduct. The final decision on any conflicts rests with the author.

## Getting Started

### Volunteer Project

**pleezer** is maintained by volunteers. We aim to review pull requests promptly, but response times may vary from a day to several weeks. If you feel a review is delayed, feel free to send a polite reminder. Peer reviews are also encouraged as the community grows.

## How to Contribute

### Reporting Bugs

1. **Check for Existing Issues**: Before reporting a bug, search the issue tracker to avoid duplicates.
2. **Create a GitHub Issue**: If the bug is new, create a [GitHub issue](https://github.com/roderickvd/pleezer/issues) and label it as "bug". Include as much detail as possible—steps to reproduce and logs are required.

### Feature Requests

1. **Submit as GitHub Issues**: If you have an idea for a new feature, submit it as a [GitHub issue](https://github.com/roderickvd/pleezer/issues) with the "enhancement" label.
2. **Be Detailed**: Provide clear details about the feature you’re proposing to help us understand and prioritize it.

### Pull Request Process

1. **Create a Branch**: Always create a new branch for each feature or bug fix. Avoid committing directly to the `main` branch. Use descriptive branch names.
2. **Test Your Code**: Ensure your code works as expected before submitting a pull request. Cross-platform testing is encouraged if possible.
3. **Update the Changelog**: Update the [CHANGELOG.md](CHANGELOG.md) file with a summary of your changes under the "Unreleased" section. Follow the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format.
4. **Open a Pull Request**: Submit a pull request against the `main` branch. Include a clear description of your changes and reference any related issues. Use descriptive and imperative commit messages.
5. **Review Process**: We will review your pull request as soon as possible, but response times can vary.
6. **Testing**: While writing new tests is encouraged, it’s not required at this stage as we do not yet have a test harness. Just make sure your code passes any existing tests.
7. **Documentation**: Contributions to documentation are highly valued. If your pull request includes changes that require documentation updates, please include them.

We will acknowledge contributors in the release notes unless they choose to opt out. Sponsoring the project via [GitHub Sponsors](https://github.com/sponsors/roderickvd) is also a meaningful way to contribute.

By contributing, you agree that your code will be licensed under the terms of the [Sustainable Use License](LICENSE.md).

### Security

Please refer to the [SECURITY.md](SECURITY.md) file for information on how to report security vulnerabilities. Do not use GitHub issues or discussions for reporting security vulnerabilities.

## Financial Contributions

If you wish to support the project financially, you can do so through the [GitHub Sponsors program for @roderickvd](https://github.com/sponsors/roderickvd). Your support is greatly appreciated and will help ensure the continued development and improvement of **pleezer**.

## CI Pipeline

Our CI pipeline, managed with [GitHub Actions](https://github.com/roderickvd/pleezer/actions), includes the following workflows:

- **Cross-Compilation**: Checks for cross-compilation on Rust stable with every push and pull request.
- **Code Quality**: Checks code formatting and linting on Rust stable using `rustfmt` and `clippy`.
- **Weekly Maintenance**: Periodically checks for compilation on Rust beta to ensure readiness for the next stable release of Rust.

Please ensure your code passes these checks before submitting a pull request.

## Automated Testing

We currently do not have a test harness. However, we encourage you to thoroughly test your changes using the built-in Rust testing framework before submitting a pull request.

## Performance Testing

While we do not have a formal performance testing process, manual testing is encouraged. Keep in mind that the minimum supported platform for **pleezer** is a Raspberry Pi 3B+ with 1GB of RAM.

## Coding Conventions

We follow Rust's idiomatic style and use `rustfmt` and `clippy` to enforce formatting and linting. Please ensure your code passes these checks before submitting a pull request.

## Documentation

Our goal is to provide thorough documentation for **pleezer**. Contributions to improve documentation are highly valued. We use Rustdoc for generating documentation. Although we currently have limited documentation, we aim to improve this over time and publish it to docs.rs when it's mature enough.

## Release Process

We aim for fast and frequent releases to ensure that improvements and fixes are delivered to users as quickly as possible. Our release process involves the following steps:

1. **Merge Pull Requests**: Ensure all relevant pull requests are merged into the `main` branch.
2. **Update Changelog**: Ensure the [CHANGELOG.md](CHANGELOG.md) file is up-to-date with the latest changes.
3. **Tag a New Release**: Create a new tag for the release.
4. **Publish to Crates.io**: Publish the new version to [crates.io](https://crates.io/crates/pleezer).

## Acknowledgments

We appreciate all contributions to **pleezer**. We will acknowledge contributors in the release notes unless they choose to opt out.
