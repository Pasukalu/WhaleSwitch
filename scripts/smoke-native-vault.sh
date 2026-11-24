#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  linux)
    scratch=$(mktemp -d "$RUNNER_TEMP/whaleswitch-keyring.XXXXXX")
    export XDG_DATA_HOME="$scratch"
    dbus-run-session -- bash scripts/smoke-native-vault.sh linux-session
    exit
    ;;
  linux-session)
    openssl rand -hex 32 | tr -d '\n' | gnome-keyring-daemon --unlock --components=secrets >/dev/null
    ;;
  macos)
    scratch=$(mktemp -d "$RUNNER_TEMP/whaleswitch-keychain.XXXXXX")
    keychain="$scratch/smoke.keychain-db"
    generated_password=$(openssl rand -hex 32)
    security create-keychain -p "$generated_password" "$keychain"
    trap 'security delete-keychain "$keychain"' EXIT
    security default-keychain -s "$keychain"
    security list-keychains -d user -s "$keychain"
    security set-keychain-settings -lut 900 "$keychain"
    security unlock-keychain -p "$generated_password" "$keychain"
    unset generated_password
    ;;
  *)
    exit 1
    ;;
esac

cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features \
  --lib storage::native_test::native_release_roundtrip -- --ignored --exact
