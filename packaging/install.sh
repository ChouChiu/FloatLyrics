#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ChouChiu
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

if (( $# < 1 || $# > 2 )); then
    echo "usage: $0 DESTDIR [BINARY]" >&2
    exit 2
fi

destdir=${1%/}
binary=${2:-target/release/floatlyrics}
prefix=${PREFIX:-/usr}
app_id=io.github.chouchiu.floatlyrics

install -Dm755 "$binary" "$destdir$prefix/bin/floatlyrics"
install -Dm644 "data/$app_id.desktop" \
    "$destdir$prefix/share/applications/$app_id.desktop"
install -Dm644 "data/$app_id.metainfo.xml" \
    "$destdir$prefix/share/metainfo/$app_id.metainfo.xml"

while IFS= read -r -d '' icon; do
    install -Dm644 "$icon" "$destdir$prefix/share/${icon#data/}"
done < <(find data/icons -type f -print0)

install -Dm644 LICENSE "$destdir$prefix/share/licenses/floatlyrics/LICENSE"

if [[ -d data/locale ]]; then
    install -d "$destdir$prefix/share/locale"
    cp -a data/locale/. "$destdir$prefix/share/locale/"
fi
