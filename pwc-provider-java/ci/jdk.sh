# Sourced by the CI scripts: installs Temurin JDK 25 for this agent under ~/.cache unless it is there, and sets
# JAVA_HOME. actions/setup-java is not used because it needs a GitHub workflow token, which this pipeline lacks.
jdk_install() {
    local os arch
    case "$(uname -s)" in
        Linux) os=linux ;;
        Darwin) os=mac ;;
        MINGW* | MSYS* | CYGWIN*) os=windows ;;
        *) echo "unsupported system $(uname -s)" >&2; return 1 ;;
    esac
    case "$(uname -m)" in
        x86_64 | amd64) arch=x64 ;;
        arm64 | aarch64) arch=aarch64 ;;
        *) echo "unsupported machine $(uname -m)" >&2; return 1 ;;
    esac
    local dir="$HOME/.cache/pwc-jdk-25-$os-$arch"
    if [ ! -x "$dir/home/bin/java" ] && [ ! -x "$dir/home/bin/java.exe" ]; then
        rm -rf "$dir"
        mkdir -p "$dir/unpacked"
        local url="https://api.adoptium.net/v3/binary/latest/25/ga/$os/$arch/jdk/hotspot/normal/eclipse"
        if [ "$os" = windows ]; then
            curl -fsSL -o "$dir/jdk.zip" "$url"
            unzip -q "$dir/jdk.zip" -d "$dir/unpacked"
            rm "$dir/jdk.zip"
        else
            curl -fsSL "$url" | tar -xz -C "$dir/unpacked"
        fi
        local top="$dir/unpacked/$(ls "$dir/unpacked")"
        if [ "$os" = mac ]; then
            mv "$top/Contents/Home" "$dir/home"
        else
            mv "$top" "$dir/home"
        fi
        rm -rf "$dir/unpacked"
    fi
    export JAVA_HOME="$dir/home" PATH="$dir/home/bin:$PATH"
    java -version
}

# The native library's file name on the platform <os>-<arch>.
library_file() {
    case "$1" in
        linux-*) echo libpwc_provider_jni.so ;;
        macos-*) echo libpwc_provider_jni.dylib ;;
        windows-*) echo pwc_provider_jni.dll ;;
    esac
}

PLATFORMS="linux-x86_64 linux-aarch64 macos-x86_64 macos-aarch64 windows-x86_64"
