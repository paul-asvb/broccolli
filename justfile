default:
    @just --list

# run the axum server locally
run:
    cargo run --bin broccolli

# run the backend and the yew frontend dev server together
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT
    (cd web && trunk serve --port 8081) &
    cargo run --bin broccolli &
    wait

# type-check and lint everything
check:
    cargo clippy --all-targets

# format code
fmt:
    cargo fmt

# build the release binary natively, without docker
build-local:
    cargo build --release

# serve the yew frontend standalone with hot reload, no auth (fast UI iteration)
web-dev:
    cd web && trunk serve --port 8081

# build the yew frontend bundle (also runs automatically as part of `cargo build`)
web-build:
    cd web && trunk build --release

# create the messages table in Turso if it doesn't exist
db-migrate:
    cargo run --bin turso_migrate

# print the messages table schema
db-check:
    cargo run --bin turso_check

# print row count and a few sample rows
db-verify:
    cargo run --bin turso_verify

# import a Telegram chat export json into Turso
import path="chat.json":
    cargo run --bin chat_import -- {{path}}

# poll the Telegram bot for updates into a local jsonl dump
telegram-dump:
    cargo run --bin telegram_dump

# build the production docker image
docker-build:
    docker build -t broccolli .

# run the production docker image locally on :8080
docker-run:
    docker run --rm -p 8080:8080 --env-file .env broccolli

# deploy to fly.io, building the docker image on fly's remote builder
deploy:
    flyctl deploy --remote-only

# deploy to fly.io, building the docker image on this machine
deploy-local:
    flyctl deploy --local-only
