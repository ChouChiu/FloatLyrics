#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ChouChiu
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
package_dir="$repo_root/packaging/aur/floatlyrics"

# makepkg defaults to $startdir/src, which collides with this Rust workspace.
export BUILDDIR="$repo_root/.makepkg/build"
export SRCDEST="$repo_root/.makepkg/sources"
export PKGDEST="$repo_root"

cd "$package_dir"
exec makepkg "$@"
