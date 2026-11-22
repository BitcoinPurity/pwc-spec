#!/usr/bin/env bash
# Assembles the jar with the native libraries of every platform from natives/<platform>/ and checks it.
# With --publish <tag>, also publishes it to GitHub Packages; <tag> must be pwc-provider-java-v<version>.
# Usage, from pwc-provider-java/: ci/assemble.sh [--publish <tag>]
set -euo pipefail
. "$(dirname "$0")/jdk.sh"

for p in $PLATFORMS; do
    test -f "natives/$p/$(library_file "$p")" || { echo "missing natives/$p/$(library_file "$p")" >&2; exit 1; }
done
jdk_install
natives="$PWD/natives"
./gradlew jar sourcesJar --no-daemon -PnativeLibs="$natives"
version=$(./gradlew properties --no-daemon -q | sed -n 's/^version: //p')
jar="build/libs/pwc-provider-java-$version.jar"
listing=$(unzip -l "$jar")
for entry in module-info.class $(for p in $PLATFORMS; do echo "org/bitcoinpurity/pwc/provider/native/$p/$(library_file "$p")"; done); do
    grep -q " $entry\$" <<<"$listing" || { echo "$jar lacks $entry" >&2; exit 1; }
done
echo "$jar: module-info.class and the libraries of $PLATFORMS"

if [ "${1:-}" = --publish ]; then
    tag="${2:?the tag}"
    [ "$tag" = "pwc-provider-java-v$version" ] || { echo "tag $tag does not match version $version" >&2; exit 1; }
    : "${PWC_PACKAGES_USER:?}" "${PWC_PACKAGES_TOKEN:?}"
    ./gradlew publish --no-daemon -PnativeLibs="$natives"
fi
