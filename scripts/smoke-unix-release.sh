#!/usr/bin/env bash
set -euo pipefail

target=$1
package=$2
scratch=$(mktemp -d "$RUNNER_TEMP/whaleswitch-smoke.XXXXXX")
mountpoint=""
app_pid=""

clean_up() {
  if [[ -n "$app_pid" ]]; then
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  if [[ -n "$mountpoint" ]]; then
    hdiutil detach "$mountpoint"
  fi
}
trap clean_up EXIT

case "$target" in
  macos-arm64|macos-x64)
    mkdir "$scratch/mount"
    hdiutil attach -readonly -nobrowse -mountpoint "$scratch/mount" "$package"
    mountpoint="$scratch/mount"
    app="$scratch/WhaleSwitch.app"
    ditto "$mountpoint/WhaleSwitch.app" "$app"
    if codesign --verify --strict "$app" 2>/dev/null; then
      codesign -dv "$app" 2>&1
    else
      echo 'No verified application-bundle code signature.'
    fi
    if xcrun stapler validate "$app" >"$scratch/notarization.log" 2>&1; then
      echo 'Stapled Apple notarization ticket validated.'
    else
      echo 'No validated stapled Apple notarization ticket.'
    fi
    executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist")
    binary="$app/Contents/MacOS/$executable"
    ;;
  linux-x64)
    installed=$(dpkg-deb --field "$package" Package)
    [[ "$(dpkg-query -W -f='${Status}' "$installed")" == 'install ok installed' ]]
    binary=$(command -v whaleswitch)
    ;;
  *)
    echo 'Unknown native smoke-test target.' >&2
    exit 1
    ;;
esac

[[ -x "$binary" ]]
"$binary" >"$scratch/launch.log" 2>&1 &
app_pid=$!
sleep 8
if ! kill -0 "$app_pid" 2>/dev/null; then
  echo "WhaleSwitch exited before the $target startup check completed." >&2
  cat "$scratch/launch.log" >&2
  exit 1
fi
echo "The published $target package launched successfully."
