#!/usr/bin/env bash
set -euo pipefail
# Nothing ends silently: a step that fails says where.
trap 'echo "x: stopped at line $LINENO: $BASH_COMMAND" >&2' ERR
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
  ghostty)
    # libghostty for the terminal pane (ADR 0004), on the Mac: Ghostty's source at a pinned tag, built with Zig,
    # without Sentry (Ghostty's macOS default) and without its app. Prints the library's path (1.4: libghostty-internal.a).
    tag=v1.4.0 src="$target_dir/ghostty-$tag"
    # Not built yet is no failure: then the folder is missing and find says so.
    lib=$(find "$src/macos/GhosttyKit.xcframework" -name 'lib*.a' 2>/dev/null | head -1 || true)
    if [[ -z "$lib" ]]; then
      command -v zig >/dev/null || { echo "ghostty: needs Zig 0.17 or later (brew install zig)" >&2; exit 1; }
      zv=$(zig version)
      [[ "$(printf '%s\n' 0.17.0 "$zv" | sort -V | head -1)" == 0.17.0 ]] || { echo "ghostty: Zig $zv is too old, $tag needs 0.17 or later" >&2; exit 1; }
      # Ghostty's shaders need Apple's Metal compiler: part of Xcode, a separate download since Xcode 26.
      xcrun -sdk macosx metal -v >/dev/null 2>&1 || {
        echo "ghostty: needs Xcode's Metal toolchain: xcodebuild -downloadComponent MetalToolchain" >&2
        echo "  (with Xcode selected: xcode-select -p should show Xcode.app, not CommandLineTools)" >&2
        exit 1
      }
      [[ -d "$src" ]] || git clone -q --depth 1 --branch "$tag" https://github.com/ghostty-org/ghostty "$src" >&2
      (cd "$src" && zig build -Doptimize=ReleaseFast -Demit-xcframework=true -Dxcframework-target=native \
        -Demit-macos-app=false -Dsentry=false >&2)
      lib=$(find "$src/macos/GhosttyKit.xcframework" -name 'lib*.a' | head -1 || true)
      [[ -n "$lib" ]] || { echo "ghostty: the build left no library in $src/macos/GhosttyKit.xcframework" >&2; exit 1; }
    fi
    echo "$lib"
    ;;
  bundle)
    if [[ "$(uname)" == Darwin ]]; then
      GHOSTTY_LIB=$("$0" ghostty); export GHOSTTY_LIB
    fi
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
    echo "usage: ./x fmt | lint | test | spec | mac | matrix | check | ghostty | run | bundle | release <version>" >&2
    exit 1
    ;;
esac
