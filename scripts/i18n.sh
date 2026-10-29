#!/bin/sh
# Refresh the translation template (po/memeplex.pot) from the app, its UI files, the desktop file
# and the metainfo, then merge it into every catalog in po/LINGUAS. Run it after adding or
# changing text and commit the result with the change. Needs GNU gettext 0.24 or newer, for
# xgettext's Rust support.
set -eu
cd "$(dirname "$0")/.."
export GETTEXTDATADIRS="$PWD/po" # po/its: what of the metainfo is translated

pot=po/memeplex.pot
x() {
  # --force-po: a template even before the app's Rust has text to translate.
  xgettext --force-po --from-code=UTF-8 --add-comments=Translators: --sort-by-file \
    --package-name=Memeplex --msgid-bugs-address=https://github.com/AperteDev/memeplex/issues --output="$pot" "$@"
}

x -L Rust -k -kgettext -kngettext:1,2 -kpgettext:1c,2 -kgettext_noop memeplex/linux/app/src/*.rs
# Blueprint's _("…") and C_("context", "…") read like C.
x --join-existing -L C -k -k_ -kC_:1c,2 memeplex/linux/app/ui/*.blp
x --join-existing memeplex/linux/data/dev.aperte.Memeplex.desktop \
  memeplex/linux/data/dev.aperte.Memeplex.metainfo.xml

for lang in $(grep -v '^#' po/LINGUAS); do
  if [ -f "po/$lang.po" ]; then
    msgmerge --quiet --update --backup=none --previous "po/$lang.po" "$pot"
  else
    msginit --no-translator --input="$pot" --locale="$lang.UTF-8" --output-file="po/$lang.po"
  fi
done
