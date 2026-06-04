# Local podman-compose testing

This directory contains a local `podman-compose` setup for running Thufir with the same container contract used by the root `Makefile`: build `../Containerfile`, load runtime secrets from `.env`, and mount `thufir.toml` at `/etc/thufir/thufir.toml`.

## First-time setup

```bash
cd podman
cp .env.example .env
$EDITOR .env
$EDITOR thufir.toml
```

Fill in these required `.env` secret values:

- `DISCORD_TOKEN`: Discord bot token
- `VL_USERNAME`: VolumeLeaders username
- `VL_PASSWORD`: VolumeLeaders password

Fill in non-secret settings in `thufir.toml`, especially `discord.guild_id`. Command channel allow-lists live under `[commands.<command>].allowed_channels`; leave a list empty to allow the command in any channel in the configured guild.

Example:

```toml
[discord]
guild_id = 123456789012345678

[commands.trade_dashboard]
allowed_channels = [234567890123456789]
```

`THUFIR_` variables can still override matching TOML values for deployments that prefer env-driven non-secrets, but the local compose setup keeps `.env` for secrets only.

## Run the bot

```bash
cd podman
podman-compose -f compose.yml up --build
```

Run it in the background:

```bash
podman-compose -f compose.yml up --build -d
podman-compose -f compose.yml logs -f thufir
```

Stop and remove the local container:

```bash
podman-compose -f compose.yml down
```

## Notes

- `podman/.env` and `podman/thufir.toml` are ignored by git because they contain local-only values. Keep secrets in `.env`; keep non-secrets in `thufir.toml`.
- The compose service mounts `./thufir.toml` read-only at `/etc/thufir/thufir.toml` and starts Thufir with `--config /etc/thufir/thufir.toml`.
- If you only change Rust code, rerun `podman-compose -f compose.yml up --build` so the image rebuilds from `../Containerfile`.
- If `podman compose up` shows an old configuration error, rebuild the local image with `podman compose -f compose.yml up --build`. The plain `up` command reuses the existing `thufir:dev` image.
- If startup reports that `discord.guild_id` is unset, edit `thufir.toml` and replace the placeholder `guild_id = 0` with your Discord server ID.
