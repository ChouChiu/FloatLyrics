#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ChouChiu
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

prepare_only=false
if [[ ${1:-} == --prepare-only ]]; then
    prepare_only=true
    shift
fi
if (( $# != 2 )); then
    echo "usage: $0 [--prepare-only] PACKAGE VERSION" >&2
    exit 2
fi

package_selection=$1
version=$2

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
"$repo_root/packaging/update-aur-checksum.sh" "$package_selection" "$version"

case $package_selection in
    all)
        packages=(floatlyrics floatlyrics-bin)
        directories=("$repo_root" "$repo_root/packaging/aur/floatlyrics-bin")
        diff_paths=(PKGBUILD .SRCINFO packaging/aur/floatlyrics-bin/PKGBUILD packaging/aur/floatlyrics-bin/.SRCINFO)
        ;;
    floatlyrics)
        packages=(floatlyrics)
        directories=("$repo_root")
        diff_paths=(PKGBUILD .SRCINFO)
        ;;
    floatlyrics-bin)
        packages=(floatlyrics-bin)
        directories=("$repo_root/packaging/aur/floatlyrics-bin")
        diff_paths=(packaging/aur/floatlyrics-bin/PKGBUILD packaging/aur/floatlyrics-bin/.SRCINFO)
        ;;
esac
for index in "${!packages[@]}"; do
    directory=${directories[$index]}
    (
        cd "$directory"
        makepkg --printsrcinfo | diff -u .SRCINFO -
        namcap PKGBUILD
    )
done

git -C "$repo_root" diff -- "${diff_paths[@]}"

if [[ $prepare_only == true ]]; then
    echo "AUR package files prepared; publishing skipped."
    exit 0
fi

read -r -p "Publish $package_selection $version to AUR? [y/N] " reply
if [[ ! $reply =~ ^[Yy]$ ]]; then
    echo "Publishing cancelled."
    exit 0
fi

ssh -o BatchMode=yes aur@aur.archlinux.org help >/dev/null

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT
for index in "${!packages[@]}"; do
    package=${packages[$index]}
    source_directory=${directories[$index]}
    aur_directory="$work_dir/$package"

    git -c init.defaultBranch=master clone \
        "ssh://aur@aur.archlinux.org/$package.git" "$aur_directory"
    install -m644 "$source_directory/PKGBUILD" "$aur_directory/PKGBUILD"
    install -m644 "$source_directory/.SRCINFO" "$aur_directory/.SRCINFO"
    git -C "$aur_directory" add PKGBUILD .SRCINFO
    git -C "$aur_directory" diff --cached --check
done

for package in "${packages[@]}"; do
    aur_directory="$work_dir/$package"

    if git -C "$aur_directory" diff --cached --quiet; then
        echo "$package is already up to date."
        continue
    fi

    git -C "$aur_directory" commit -m "Update to $version"
    git -C "$aur_directory" push origin master
done
