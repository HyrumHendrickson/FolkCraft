# Security Policy

## Supported versions

Folk Craft is under active development. Only the latest commit on `main` receives security fixes.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through
[GitHub private vulnerability reporting](https://github.com/HyrumHendrickson/FolkCraft/security/advisories/new).

Include the affected commit or version, a description of the impact, and steps to reproduce. You can expect an
acknowledgement within 7 days and a status update within 30 days. Reporters are credited in the advisory unless they
ask otherwise.

## Scope

In scope:

- The local multiplayer protocol (`src/net.rs`, `src/lobby.rs`, `src/game/session.rs`): memory safety, crashes or
  resource exhaustion triggered by a malformed or hostile packet, and any way for a guest to affect the host's machine.
- Loading of save files (`src/save.rs`) and world files from untrusted sources.

Out of scope: gameplay exploits that only affect a world you host yourself, and issues in third-party dependencies
that are already tracked upstream (these are monitored through Dependabot and `cargo audit`).

## Multiplayer exposure

A hosted game listens on TCP port 24680 on all interfaces and announces itself by UDP broadcast on port 24681. The
protocol is not encrypted or authenticated. Host only on networks you trust, and do not forward these ports to the
public internet.
