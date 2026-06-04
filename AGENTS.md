# Thufir - Agent and Developer Reference

Thufir is a single-guild Discord bot written in Rust. It integrates with VolumeLeaders to surface market data (trades, clusters, chart levels, cluster bombs) as Discord slash command responses. The bot is designed for one guild only; global command registration is not in scope.

## Rust Version

Pinned to **1.96.0** via `rust-toolchain.toml`. The MSRV is also 1.96. Keep `rust-toolchain.toml`, `Cargo.toml` `rust-version`, and the CI `msrv` job in sync when bumping.

## Project Layout

```text
src/
  main.rs              - startup sequence (see below)
  lib.rs               - crate root, module declarations, re-exports
  error.rs             - thiserror Error enum + Result<T> alias
  config.rs            - figment-based config (env + optional TOML)
  observability.rs     - tracing subscriber init
  state.rs             - AppState (shared across commands)
  data_sources/        - VolumeLeadersManager (session, retry, login)
  dashboard/           - embed renderer (truncation, formatting)
  commands/            - poise command handlers (ping, trade_dashboard)
tests/
  cli.rs               - integration tests (assert_cmd, no live secrets)
Containerfile          - multi-stage UBI9 build
Makefile               - quality gate targets
.github/workflows/     - ci.yml, audit.yml
```

## Startup Sequence

`main.rs` runs these steps in order:

1. `dotenvy::dotenv().ok()` - load `.env` if present (optional, no error if missing)
2. `Cli::parse()` - clap parses `--config <path>` flag; `--help`/`--version` exit here before any secrets are needed
3. `Config::load(path)` - figment loads env vars and optional TOML
4. `init_observability(&config.bot.log_level)` - tracing subscriber with JSON format
5. `rusty_volumeleaders::resolve_credentials()` - reads VL credentials from env
6. `VolumeLeadersManager::new(username, password).await` - performs startup login
7. Build `AppState` with `Arc<RwLock<VolumeLeadersManager>>`
8. Build poise `Framework` with guild-only command registration via `register_in_guild()`
9. Build serenity client with `GatewayIntents::non_privileged()`
10. `tokio::select!` with `client.start()` and `ctrl_c()` for graceful shutdown

## Required Environment Variables

| Variable | Source | Description |
|---|---|---|
| `DISCORD_TOKEN` | Set directly | Bot token from Discord developer portal |
| `THUFIR_DISCORD__GUILD_ID` | `THUFIR_` prefix | Numeric guild (server) ID |
| `VL_USERNAME` | rusty-volumeleaders | VolumeLeaders account username |
| `VL_PASSWORD` | rusty-volumeleaders | VolumeLeaders account password |

`VL_USERNAME` and `VL_PASSWORD` are consumed by `rusty_volumeleaders::resolve_credentials()`, not by Thufir's config module directly.

Optional config file: `thufir.toml` (pass path via `--config`). All `THUFIR_` prefixed env vars override TOML values. Double underscore (`__`) separates nested keys (e.g., `THUFIR_DISCORD__GUILD_ID`).

**Never commit real values for any of these variables.**

## Configuration Defaults

| Setting | Default |
|---|---|
| `bot.log_level` | `"info"` |
| `bot.timezone` | `"America/New_York"` |
| `volume_leaders.dashboard_days` | `365` |
| `volume_leaders.dashboard_count` | `10` |

## TLS Policy

Native TLS only. The `reqwest` dependency uses the `native-tls` feature. `rustls` and AWS TLS are not used. The container runtime (UBI9 minimal) provides `libssl.so.3` and `libcrypto.so.3` from the system OpenSSL package.

## Container Build

Multi-stage build using Red Hat hardened images:

- **Builder**: `registry.access.redhat.com/ubi9/ubi:latest` - installs gcc, openssl-devel, perl-FindBin, pkg-config, then Rust 1.96.0 via rustup
- **Runtime**: `registry.access.redhat.com/ubi9/ubi-minimal:latest` - copies binary to `/usr/bin/thufir`, runs as UID 1001 (non-root)

Build and run locally:

```bash
make container-build
make container-run          # requires .env file with secrets
make container-run-config   # mounts thufir.toml at /etc/thufir/thufir.toml
```

## Quality Gates

`make check` runs all gates in sequence:

| Target | Command |
|---|---|
| `fmt` | `cargo fmt --all --check` |
| `clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| `test` | `cargo test --workspace --all-features` |
| `doc` | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items` |
| `coverage` | `cargo llvm-cov` with 90% line coverage threshold |
| `patch-coverage` | `diff-cover` with 95% patch coverage threshold against `main` |

Additional targets (not in `check`):

| Target | Purpose |
|---|---|
| `make audit` | `cargo audit` for known CVEs |
| `make machete` | `cargo machete` for unused dependencies |
| `make integration` | Runs `#[ignore]`-tagged tests (requires live credentials, not run in CI) |

All tests in CI use mocks (mockito, mockall). No live credentials are required to pass CI.

## CI Workflows

`.github/workflows/ci.yml` runs on push to `main` and all PRs. Jobs:

- `fmt` - formatting check
- `clippy` - lint with `-D warnings`
- `test` - all tests (mock-only, no secrets)
- `msrv` - `cargo check --locked` on Rust 1.96
- `doc` - docs with `-D warnings`
- `coverage` - llvm-cov with 90% threshold, uploads to codecov

`.github/workflows/audit.yml` runs weekly and on `Cargo.lock`/`Cargo.toml` changes.

## Testing Approach

All tests use mocks. No live Discord tokens, VolumeLeaders credentials, or network calls in CI.

- **Unit tests**: inline `#[cfg(test)] mod tests` in each module
- **HTTP mocking**: mockito for VolumeLeaders API responses
- **Trait mocking**: mockall for internal interfaces
- **CLI tests**: `assert_cmd` in `tests/cli.rs` (verifies `--help` exits 0 without secrets)
- **Async tests**: `#[tokio::test]`

The `rusty-volumeleaders` crate's `test-support` feature provides `test_session()` and `datatables_body()`. Use `Client::with_config(test_session(), ClientConfig { base_url: server.url(), ..Default::default() })` to point the client at a mockito server. `test_client()` is `#[cfg(test)]` only inside rusty-volumeleaders and is not available to downstream crates.

## Secrets Policy

- Never commit real tokens, passwords, server IDs, or webhook URLs
- Never add secrets to CI workflow files
- Use `.env` locally (it is gitignored via `.containerignore` and should be in `.gitignore`)
- CI passes with zero secrets because all tests use mocks

## Commands

| Command | Description |
|---|---|
| `/ping` | Health check, replies "Pong!" |
| `/trade-dashboard` | Fetches VolumeLeaders data for a ticker and renders a Discord embed |

Commands are registered guild-only via `poise::builtins::register_in_guild()` at startup. The guild ID comes from `THUFIR_DISCORD__GUILD_ID`.

## Deferred (Not in MVP)

These features are explicitly out of scope for the current MVP. Do not implement them without a plan update:

- **Scheduler** - no background jobs, no `tokio-cron-scheduler`
- **Webhooks** - no incoming or outgoing webhook handling
- **Global command registration** - guild-only registration only
- **Live smoke tests** - no `#[ignore]` tests that require real credentials in CI
- **Sentry** - no error tracking integration
- **Multi-guild support** - single guild only
- **High availability / clustering** - single instance only
- **GHCR publish** - container image publishing not automated
- **release-plz** - automated releases not configured
- **Additional commands** - `/jobs`, `/status`, `/data`, `/volumeleaders` alias are deferred

## Optional Smoke Test Checklist

These steps are manual verification only. They are not required for CI and should not be automated without live credentials.

- [ ] Bot comes online in the target guild after `cargo run` with valid env vars
- [ ] `/ping` responds with "Pong!" in under 1 second
- [ ] `/trade-dashboard AAPL 30 5` returns an embed with trade data
- [ ] `/trade-dashboard INVALID 30 5` returns a user-facing error message (not a panic)
- [ ] Bot shuts down cleanly on Ctrl+C (no error output)
- [ ] Container image starts and responds to `/ping` when run with `make container-run`
