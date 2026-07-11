#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ChouChiu
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

generate_srcinfo=true
if (( $# > 1 )); then
    echo "usage: $0 [--checksum-only]" >&2
    exit 2
fi
if (( $# == 1 )); then
    if [[ $1 != --checksum-only ]]; then
        echo "usage: $0 [--checksum-only]" >&2
        exit 2
    fi
    generate_srcinfo=false
fi
if [[ $generate_srcinfo == true ]] && (( EUID == 0 )); then
    echo "refusing to run makepkg as root; use --checksum-only and generate .SRCINFO as a build user" >&2
    exit 1
fi

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

pkgver=$(sed -n 's/^pkgver=//p' PKGBUILD)
if [[ ! "$pkgver" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    echo "invalid pkgver in PKGBUILD: $pkgver" >&2
    exit 1
fi

archive=$(mktemp)
trap 'rm -f "$archive"' EXIT

curl --fail --location --retry 5 --retry-all-errors \
    --output "$archive" \
    "https://github.com/ChouChiu/FloatLyrics/archive/refs/tags/v$pkgver.tar.gz"
checksum=$(sha256sum "$archive" | cut -d ' ' -f 1)

sed -i -E "s/^sha256sums=\('[^']*'\)$/sha256sums=('$checksum')/" PKGBUILD
grep -Fxq "sha256sums=('$checksum')" PKGBUILD

if [[ $generate_srcinfo == true ]]; then
    srcinfo=$(mktemp)
    makepkg --printsrcinfo > "$srcinfo"
    mv "$srcinfo" .SRCINFO
fi
