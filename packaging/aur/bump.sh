#!/usr/bin/env bash
# Points the AUR package at a published launcher release: ./bump.sh 0.3.3
# With a .deb already on disk (the release workflow, whose draft release has no public download
# yet), the checksum comes from that file instead: ./bump.sh 0.3.3 path/to/launcher.deb
# Updates pkgver, pkgrel and sha256sums in PKGBUILD and .SRCINFO. Run it after the GitHub release
# is out, then copy both files into the AUR git repository and push (see the README).
set -euo pipefail

version="${1:?usage: $0 <version, e.g. 0.3.3>}"
version="${version#v}"
local_deb="${2:+$(realpath "$2")}"
cd "$(dirname "$0")"

# PKGBUILD owns the download URL; evaluating it with the new pkgver gives that version's URL.
# Nothing is written until the download succeeds, so a wrong version leaves both files untouched.
# shellcheck disable=SC2016
deb_url=$(sed "s/^pkgver=.*/pkgver=$version/" PKGBUILD | bash -c 'source /dev/stdin; echo "${source[0]}"')
if [ -n "$local_deb" ]; then
  sum=$(sha256sum "$local_deb" | cut -d' ' -f1)
else
  sum=$(curl -fsSL "$deb_url" | sha256sum | cut -d' ' -f1)
fi

sed -i -e "s/^pkgver=.*/pkgver=$version/" -e "s/^pkgrel=.*/pkgrel=1/" \
  -e "s/^sha256sums=.*/sha256sums=('$sum')/" PKGBUILD

sed -i -e "s|^\tpkgver = .*|\tpkgver = $version|" \
  -e "s|^\tpkgrel = .*|\tpkgrel = 1|" \
  -e "s|^\tsource = .*|\tsource = $deb_url|" \
  -e "s|^\tsha256sums = .*|\tsha256sums = $sum|" .SRCINFO

echo "PKGBUILD and .SRCINFO now point at $version ($sum)."
