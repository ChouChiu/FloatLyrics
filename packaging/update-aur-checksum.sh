#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ChouChiu
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

generate_srcinfo=true
if [[ ${1:-} == --checksum-only ]]; then
    generate_srcinfo=false
    shift
fi
if (( $# > 2 )); then
    echo "usage: $0 [--checksum-only] [PACKAGE] [VERSION]" >&2
    exit 2
fi

package_selection=${1:-all}
version=${2:-}
case $package_selection in
    all | floatlyrics | floatlyrics-bin) ;;
    *)
        echo "invalid package selection: $package_selection" >&2
        exit 2
        ;;
esac
if [[ $generate_srcinfo == true ]] && (( EUID == 0 )); then
    echo "refusing to run makepkg as root; use --checksum-only and generate .SRCINFO as a build user" >&2
    exit 1
fi

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$repo_root/packaging/aur/floatlyrics"
source_pkgbuild="$source_dir/PKGBUILD"
source_srcinfo="$source_dir/.SRCINFO"
bin_dir="$repo_root/packaging/aur/floatlyrics-bin"
bin_pkgbuild="$bin_dir/PKGBUILD"
bin_srcinfo="$bin_dir/.SRCINFO"
cd "$repo_root"

if [[ -z $version ]]; then
    version=$(sed -n 's/^pkgver=//p' "$source_pkgbuild")
fi
if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    echo "invalid package version: $version" >&2
    exit 1
fi

temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT

if [[ $package_selection == all || $package_selection == floatlyrics ]]; then
    sed -i -E "s/^pkgver=.*/pkgver=$version/" "$source_pkgbuild"
    sed -i -E 's/^pkgrel=.*/pkgrel=1/' "$source_pkgbuild"
    source_archive="$temp_dir/floatlyrics-$version.tar.gz"
    curl --fail --location --retry 5 --retry-all-errors \
        --output "$source_archive" \
        "https://github.com/ChouChiu/FloatLyrics/archive/refs/tags/v$version.tar.gz"
    source_checksum=$(sha256sum "$source_archive" | cut -d ' ' -f 1)
    sed -i -E "s/^sha256sums=\('[^']*'\)$/sha256sums=('$source_checksum')/" "$source_pkgbuild"
    grep -Fxq "sha256sums=('$source_checksum')" "$source_pkgbuild"
    if [[ $generate_srcinfo == true ]]; then
        source_srcinfo_temp="$temp_dir/floatlyrics.SRCINFO"
        (cd "$source_dir" && makepkg --printsrcinfo) > "$source_srcinfo_temp"
        mv "$source_srcinfo_temp" "$source_srcinfo"
    fi
fi

if [[ $package_selection == all || $package_selection == floatlyrics-bin ]]; then
    sed -i -E "s/^pkgver=.*/pkgver=$version/" "$bin_pkgbuild"
    sed -i -E 's/^pkgrel=.*/pkgrel=1/' "$bin_pkgbuild"
    bin_archive="$temp_dir/floatlyrics-$version.rpm"
    curl --fail --location --retry 5 --retry-all-errors \
        --output "$bin_archive" \
        "https://github.com/ChouChiu/FloatLyrics/releases/download/v$version/floatlyrics-$version-1.x86_64.rpm"
    bin_checksum=$(sha256sum "$bin_archive" | cut -d ' ' -f 1)
    sed -i -E "s/^sha256sums_x86_64=\('[^']*'\)$/sha256sums_x86_64=('$bin_checksum')/" "$bin_pkgbuild"
    grep -Fxq "sha256sums_x86_64=('$bin_checksum')" "$bin_pkgbuild"
    if [[ $generate_srcinfo == true ]]; then
        bin_srcinfo_temp="$temp_dir/floatlyrics-bin.SRCINFO"
        (cd "$bin_dir" && makepkg --printsrcinfo) > "$bin_srcinfo_temp"
        mv "$bin_srcinfo_temp" "$bin_srcinfo"
    fi
fi
