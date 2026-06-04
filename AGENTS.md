# Thufir - Agent and Developer Reference

Thufir is a single-guild Discord bot written in Rust. It integrates with VolumeLeaders to surface market data as Discord slash command responses. Global command registration and multi-guild behavior are out of scope.

## Knowledge Base Maintenance

Keep this file updated whenever code, configuration, CI, commands, container policy, or deferred scope changes. Treat `AGENTS.md` as a living architecture spec, not a one-time generated note.

Update triggers:

- Rust version/MSRV changes: sync `rust-toolchain.toml`, `Cargo.toml` `rust-version`, CI `msrv`, and this file.
- Startup flow changes in `src/main.rs`: update the startup sequence.
- Config fields, defaults, or env vars change in `src/config.rs`: update required variables and defaults.
- New slash commands or command behavior changes in `src/commands/`: update the command list and scope notes.
- VolumeLeaders session, retry, or credential handling changes in `src/data_sources/`: update integration notes.
- Makefile, CI jobs, coverage thresholds, audit policy, or container targets change: update quality gates and workflows.
- Deferred MVP scope changes: move items into the active sections or remove them from `Deferred`.

Child `AGENTS.md` files are not warranted at current scale. Revisit if `src/commands/` grows past five command files, `src/data_sources/` adds another integration, or a subdirectory develops rules that differ from this root file.

## Rust Version

Pinned to **1.96.0** via `rust-toolchain.toml`. MSRV is also 1.96. Keep `rust-toolchain.toml`, `Cargo.toml` `rust-version`, CI `msrv`, `Containerfile`, and this file in sync when bumping.

## Project Layout

```text
src/
  main.rs              - startup sequence
  lib.rs               - crate root, module declarations, Error/Result re-export
  error.rs             - thiserror Error enum + Result<T> alias
  config.rs            - figment config from defaults, TOML, env
  observability.rs     - JSON tracing subscriber init
  state.rs             - AppState shared across commands
  data_sources/        - VolumeLeadersManager session, retry, login
  dashboard.rs         - Discord embed renderer, truncation, formatting
  commands/            - poise command handlers
tests/cli.rs           - assert_cmd integration tests, no secrets
Containerfile          - multi-stage Red Hat hardened image build
Makefile               - quality gate targets
.github/workflows/     - ci.yml, audit.yml
```

## Startup Sequence

`src/main.rs` runs these steps in order:

1. `dotenvy::dotenv().ok()` loads `.env` if present.
2. `Cli::parse()` handles `--config <path>`; `--help` and `--version` exit before secrets are needed.
3. `Config::load(path)` loads defaults, optional TOML, and env overrides.
4. `init_observability(&config.bot.log_level)` initializes JSON tracing.
5. `rusty_volumeleaders::resolve_credentials()` reads `VL_USERNAME` and `VL_PASSWORD`.
6. `VolumeLeadersManager::new(username, password).await` performs startup login.
7. `AppState` stores `Arc<RwLock<VolumeLeadersManager>>`.
8. Poise `Framework` registers commands in one guild via `register_in_guild()`.
9. Serenity client uses `GatewayIntents::non_privileged()`.
10. `tokio::select!` runs `client.start()` or graceful shutdown from Ctrl+C/SIGTERM.

## Configuration

Secret variables are env-only:

| Variable | Source | Description |
|---|---|---|
| `DISCORD_TOKEN` | Set directly | Discord bot token |
| `VL_USERNAME` | rusty-volumeleaders | VolumeLeaders username |
| `VL_PASSWORD` | rusty-volumeleaders | VolumeLeaders password |

`VL_USERNAME` and `VL_PASSWORD` are consumed by `rusty_volumeleaders::resolve_credentials()`, not by `src/config.rs`. `DISCORD_TOKEN` is loaded directly from the process environment and is ignored if present in TOML.

Non-secret settings belong in the optional `thufir.toml`, passed via `--config`. `THUFIR_` env vars may override TOML for deployments that prefer environment-driven non-secrets; double underscore separates nested keys.

| Setting | TOML path | Env override | Description |
|---|---|---|---|
| `discord.guild_id` | `[discord] guild_id` | `THUFIR_DISCORD__GUILD_ID` | Numeric guild ID |
| `commands.ping.allowed_channels` | `[commands.ping] allowed_channels` | `THUFIR_COMMANDS__PING__ALLOWED_CHANNELS` | Optional `/ping` channel allow-list |
| `commands.trade_dashboard.allowed_channels` | `[commands.trade_dashboard] allowed_channels` | `THUFIR_COMMANDS__TRADE_DASHBOARD__ALLOWED_CHANNELS` | Optional `/trade-dashboard` channel allow-list |

Command channel allow-lists are configured under `[commands.<command>].allowed_channels`. Empty lists are the default and mean unrestricted within the configured guild. Denials are ephemeral and happen before command-specific validation or external data calls.

Defaults:

| Setting | Default |
|---|---|
| `bot.log_level` | `"info"` |
| `bot.timezone` | `"America/New_York"` |
| `volume_leaders.dashboard_days` | `365` |
| `volume_leaders.dashboard_count` | `10` |

## Commands

| Command | File | Notes |
|---|---|---|
| `/ping` | `src/commands/ping.rs` | Health check, replies `Pong!` |
| `/trade-dashboard` | `src/commands/trade_dashboard.rs` | Validates ticker/days/count, fetches four VolumeLeaders datasets, renders embed |

Commands are registered guild-only. Add new commands to `commands::get_commands()`, `CommandsConfig`, `AppState` command access, channel checks, and this table.

## VolumeLeaders and Dashboard

- `VolumeLeadersManager` stores credentials and a `rusty_volumeleaders::Client` behind `Arc<RwLock<_>>`.
- `call_with_retry!` retries exactly once after session expiry by re-authenticating.
- `/trade-dashboard` validates before external work, defers the Discord response, then fetches trades, clusters, levels, and cluster bombs.
- `dashboard.rs` enforces Discord limits: 1024 characters per field and 6000 total characters.

## TLS and Container Policy

Native TLS only. `reqwest` uses `native-tls`, `serenity` uses `default_native_tls`, and Red Hat hardened images supply the runtime TLS stack. Do not switch to rustls or AWS TLS without a plan update.

Container build:

- Builder: `registry.access.redhat.com/hi/rust:1.95-builder`, installs Rust 1.96.0 with rustup, then builds the release binary with Rust 1.96.
- Runtime: `registry.access.redhat.com/hi/core-runtime:2.42`, copies OpenSSL runtime libraries and the binary to `/usr/bin/thufir`, runs as the image default non-root user.

## Quality Gates

`make check` runs `fmt`, `clippy`, `test`, `doc`, `coverage`, and `patch-coverage`.

| Target | Command or threshold |
|---|---|
| `fmt` | `cargo fmt --all --check` |
| `clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| `test` | `cargo test --workspace --all-features` |
| `doc` | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items` |
| `coverage` | `cargo llvm-cov`, 90% line coverage for local `make check` |
| `patch-coverage` | `diff-cover`, 95% patch coverage against `main` |

Additional targets: `make audit`, `make machete`, `make integration`, `make container-build`, `make container-run`, `make container-run-config`.

## CI and Testing

`.github/workflows/ci.yml` runs fmt, clippy, test, MSRV 1.96 `cargo check --locked`, docs with warnings denied, and llvm-cov LCOV generation/upload without a blocking coverage threshold while the baseline catches up. `.github/workflows/audit.yml` runs weekly and on `Cargo.lock` or `Cargo.toml` changes.

All CI tests use mocks and require zero live secrets. Unit tests live inline in `#[cfg(test)] mod tests`; `tests/cli.rs` verifies CLI help/version without secrets. Use mockito for VolumeLeaders HTTP responses, mockall for trait mocks, assert_cmd for CLI tests, and `#[tokio::test]` for async tests. The `rusty-volumeleaders` `test-support` feature provides `test_session()` and `datatables_body()`.

## Secrets Policy

- Never commit real tokens, passwords, guild IDs, webhook URLs, or API keys.
- Never add secrets to CI workflow files.
- Use `.env` locally; CI must continue passing with mocks and no secrets.

## Deferred

Do not implement these without a plan update: scheduler or `tokio-cron-scheduler`, webhooks, global command registration, live smoke tests in CI, Sentry, multi-guild support, high availability or clustering, GHCR publish automation, release-plz, or extra commands such as `/jobs`, `/status`, `/data`, or `/volumeleaders` alias.

## Optional Manual Smoke Test

With valid local secrets only: bot comes online, `/ping` responds under 1 second, `/trade-dashboard AAPL 30 5` returns an embed, invalid tickers return user-facing errors, Ctrl+C shuts down cleanly, and the container starts via `make container-run`.
