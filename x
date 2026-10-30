#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

mac_target=aarch64-apple-darwin
target_dir="${CARGO_TARGET_DIR:-target}"

case "${1:-help}" in
  fmt) cargo fmt --all ;;
  lint)
    cargo fmt --all --check
    cargo clippy -q --workspace --all-targets -- -D warnings
    ;;
  test)
    mkdir -p "$target_dir"
    cargo test --workspace 2>&1 | tee "$target_dir/test-results.txt" | grep -E "^test result|FAILED|panicked"
    status=${PIPESTATUS[0]}
    "$0" spec
    exit "$status"
    ;;
  spec) python3 -I ../tools/specstatus.py spec "$target_dir/test-results.txt" ;;   # red/green status of the spec
  mac)
    if [[ "$(uname)" == Darwin ]]; then exit 0; fi
    rustup target list --installed | grep -q $mac_target || rustup target add $mac_target
    command -v clang >/dev/null || sudo apt-get install -y -q clang >/dev/null || { echo "mac: needs clang" >&2; exit 1; }
    # ring's build script compiles C for the mac target; without an Apple SDK we build it freestanding
    # against a stub TargetConditionals.h. Objects are never linked by `cargo check`.
    stub="$(realpath -m "$target_dir")/mac-stub"
    mkdir -p "$stub"
    printf '#define TARGET_OS_MAC 1\n#define TARGET_OS_OSX 1\n#define TARGET_OS_IPHONE 0\n' > "$stub/TargetConditionals.h"
    CC_aarch64_apple_darwin=clang AR_aarch64_apple_darwin=ar \
      CFLAGS_aarch64_apple_darwin="-ffreestanding -nostdlibinc -DRING_CORE_NOSTDLIBINC=1 -isystem $stub" \
      cargo clippy -q --workspace --all-targets --target $mac_target -- -D warnings
    ;;
  matrix) python3 -I ../tools/matrix2md.py docs/comparison.html docs/features.md ;;   # after editing the comparison page
  check) "$0" lint && "$0" test && "$0" mac && echo "check: ok" ;;
  run)
    shift
    # On the Mac as Yggdrasil.app, so it has its icon and the ember accent in standard windows (../docs/design.md).
    if [[ "$(uname)" == Darwin ]]; then
      app=$("$0" bundle | tail -1)
      open -n "$app" --args "$@"
    else
      cargo run -q -p yggdrasil --release -- "$@"
    fi
    ;;
  bundle)
    cargo build -p yggdrasil --release ${2:+--target $2}
    app="$target_dir/Yggdrasil.app"
    rm -rf "$app" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$target_dir"/${2:+$2/}release/yggdrasil "$app/Contents/MacOS/"
    cp crates/app/Info.plist "$app/Contents/"
    v=$(perl -ne 'print $1 if /^version = "(.*)"/' crates/app/Cargo.toml)
    perl -pi -e "s#(<key>CFBundleShortVersionString</key><string>)[^<]*#\${1}$v#; s#(<key>CFBundleVersion</key><string>)[^<]*#\${1}$v#" "$app/Contents/Info.plist"
    if command -v iconutil >/dev/null; then
      set="$target_dir/Yggdrasil.iconset"
      rm -rf "$set" && mkdir -p "$set"
      for s in 16 32 128 256 512; do
        sips -z $s $s assets/icon.png --out "$set/icon_${s}x${s}.png" >/dev/null
        sips -z $((s * 2)) $((s * 2)) assets/icon.png --out "$set/icon_${s}x${s}@2x.png" >/dev/null
      done
      iconutil -c icns "$set" -o "$app/Contents/Resources/Yggdrasil.icns"
    fi
    [ -x ../tools/mac-accent.sh ] && ../tools/mac-accent.sh "$app"
    # Signed like the release (the "Pantheon Mac" certificate when it's there), so macOS keeps permissions such as Full
    # Disk Access across builds and for the installed copy; ad hoc gives every build a new identity.
    id=${SIGN_IDENTITY:-}
    if [ -z "$id" ] && security find-identity -p codesigning 2>/dev/null | grep -q '"Pantheon Mac"'; then id="Pantheon Mac"; fi
    command -v codesign >/dev/null && codesign --force --deep --sign "${id:--}" "$app" 2>/dev/null || true
    echo "$app"
    ;;
  release)
    # ./x release 0.2.0 (on the Mac): version, CHANGELOG section, checks, signed bundle → yggdrasil-release + the tap's cask.
    v=${2:?usage: ./x release <version>}
    notes=$(awk '/^## Unreleased/{on=1; next} /^## /{on=0} on' CHANGELOG.md | sed '/./,$!d')
    [ -n "$(printf '%s' "$notes" | tr -d '[:space:]')" ] || { echo 'No release notes under "## Unreleased" in CHANGELOG.md.' >&2; exit 1; }
    "$0" check
    perl -pi -e 's/^version = ".*"/version = "'"$v"'"/' crates/app/Cargo.toml crates/core/Cargo.toml
    cargo update -q -w
    awk -v v="$v" -v d="$(date +%F)" '/^## Unreleased/{print; print ""; print "## " v " (" d ")"; next} {print}' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md
    app=$("$0" bundle | tail -1)
    ../tools/mac-release.sh Yggdrasil "$v" "$app"
    echo "Commit the version: git add -A && git commit -m \"chore: release $v\""
    ;;
  *)
    echo "usage: ./x fmt | lint | test | spec | mac | matrix | check | run | bundle | release <version>" >&2
    exit 1
    ;;
esac
