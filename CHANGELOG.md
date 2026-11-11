# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- MIT license, security policy, contributing guide, code of conduct, issue and pull request templates.
- Continuous integration (build, test, CodeQL, dependency audit) and Dependabot updates.

### Fixed

- Removed the `dead_code` warning for `terrain::nearest_altar`, which only the tests use (crows call `nearest_altar_beyond`).
