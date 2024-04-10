#!/bin/sh
set -eu

echo_latest_stable_version() {
    # https://gist.github.com/lukechilds/a83e1d7127b78fef38c2914c4ececc3c#gistcomment-2758860
    version="$(curl -fsSLI -o /dev/null -w "%{url_effective}" https://github.com/lapce/tiron/releases/latest)"
    version="${version#https://github.com/lapce/tiron/releases/tag/v}"
    echo "${version}"
}

os() {
    uname="$(uname)"
    case $uname in
    Linux) echo linux ;;
    Darwin) echo darwin ;;
    FreeBSD) echo freebsd ;;
    *) echo "$uname" ;;
    esac
}

arch() {
    uname_m=$(uname -m)
    case $uname_m in
    aarch64) echo arm64 ;;
    x86_64) echo amd64 ;;
    armv7l) echo armv7 ;;
    *) echo "$uname_m" ;;
    esac
}

main() {
    OS=${OS:-$(os)}
    ARCH=${ARCH:-$(arch)}
    VERSION=$(echo_latest_stable_version)
    curl -L "https://github.com/lapce/tiron/releases/download/v$VERSION/tiron-${VERSION}-${OS}-${ARCH}.gz" | sudo sh -c 'gzip -d > /usr/local/bin/tiron' && sudo chmod +x /usr/local/bin/tiron
}

main "$@"