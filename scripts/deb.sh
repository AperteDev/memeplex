#!/bin/sh
# Package a staged tree (`make -C memeplex/linux stage`, the tarball's content) as a .deb. Run it
# on the Ubuntu release the package is for: dpkg-shlibdeps takes the dependencies from the
# libraries the binary links to, as that release names them.
#
#   scripts/deb.sh <staged tree> <version> <out dir>
set -eu

stage=$1 version=$2 out=$3
arch=$(dpkg --print-architecture)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cp -a "$stage" "$work/root"
# dpkg-shlibdeps reads a source package's control file from the current directory.
mkdir -p "$work/src/debian"
printf 'Source: memeplex\n\nPackage: memeplex\nArchitecture: any\n' > "$work/src/debian/control"
depends=$(cd "$work/src" && dpkg-shlibdeps -O "$work/root/usr/bin/memeplex-linux" |
  sed -n 's/^shlibs:Depends=//p')

mkdir -p "$work/root/DEBIAN"
cat > "$work/root/DEBIAN/control" <<EOF
Package: memeplex
Version: $version
Architecture: $arch
Maintainer: SelfRef <git@selfref.dev>
Installed-Size: $(du -sk "$work/root" | cut -f1)
Depends: $depends
Section: gnome
Priority: optional
Homepage: https://github.com/AperteDev/memeplex
Description: Find memes by what is in them
 Memeplex is a library for your memes, GIFs and reels. It indexes your meme
 folders on the device, the text in images and what they show, so you can
 search them by description, then copy, drag or share the right one.
EOF

mkdir -p "$out"
dpkg-deb --root-owner-group --build "$work/root" "$out/memeplex_${version}_$arch.deb"
