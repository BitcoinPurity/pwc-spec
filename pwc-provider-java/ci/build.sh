#!/usr/bin/env bash
# Builds and tests the Java SDK on this agent, then copies its native library to natives/<platform>/.
# Usage, from pwc-provider-java/: ci/build.sh <platform>, with <platform> as in Native.platform().
set -euo pipefail
platform="$1"
. "$(dirname "$0")/jdk.sh"

case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) ;;
    *)
        # The absolute path bash resolves, because the JVM does not expand entries such as ~/.cargo/bin in PATH.
        CARGO="$(command -v cargo)"
        export CARGO
        echo "cargo: $CARGO; PATH entries with cargo: $(echo "$PATH" | tr ':' '\n' | grep cargo | tr '\n' ' ')"
        ;;
esac

(cd native && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings)
jdk_install
./gradlew build --no-daemon
mkdir -p "natives/$platform"
cp "native/target/release/$(library_file "$platform")" "natives/$platform/"
