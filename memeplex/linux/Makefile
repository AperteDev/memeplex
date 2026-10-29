# Per-user install for development; `make flatpak` builds and installs the Flatpak instead.
PREFIX ?= $(HOME)/.local
BINDIR = $(PREFIX)/bin
DATADIR = $(PREFIX)/share
ICONS = $(DATADIR)/icons/hicolor
LOCALE = $(DATADIR)/locale
ID = dev.aperte.Memeplex
# Translations (see scripts/i18n.sh); po/its says what of the metainfo is translated.
PO = ../../po
export GETTEXTDATADIRS = $(abspath $(PO))
CARGO_BUILD = cargo build --manifest-path ../../Cargo.toml

install:
	$(CARGO_BUILD) --release -p memeplex-linux
	$(MAKE) install-data

# Binary and data files of an already built release; the Flatpak build uses this too. DESTDIR
# stages them for a package. The desktop file and metainfo get their translations; the catalogs
# go where the app looks for them (../share/locale from its binary).
install-data:
	install -Dm755 ../../target/release/memeplex-linux $(DESTDIR)$(BINDIR)/memeplex-linux
	mkdir -p $(DESTDIR)$(DATADIR)/applications $(DESTDIR)$(DATADIR)/metainfo
	msgfmt --desktop --template=data/$(ID).desktop -d $(PO) \
	  -o $(DESTDIR)$(DATADIR)/applications/$(ID).desktop
	msgfmt --xml --template=data/$(ID).metainfo.xml -d $(PO) \
	  -o $(DESTDIR)$(DATADIR)/metainfo/$(ID).metainfo.xml
	for lang in $$(grep -v '^#' $(PO)/LINGUAS); do \
	  mkdir -p $(DESTDIR)$(LOCALE)/$$lang/LC_MESSAGES; \
	  msgfmt -o $(DESTDIR)$(LOCALE)/$$lang/LC_MESSAGES/memeplex.mo $(PO)/$$lang.po; \
	done
	install -Dm644 data/$(ID).svg $(DESTDIR)$(ICONS)/scalable/apps/$(ID).svg

# The system-wide tree the tarball and the .deb are made of, in DESTDIR:
#   make stage PREFIX=/usr DESTDIR=$PWD/../../target/stage
stage:
	$(CARGO_BUILD) --release --locked -p memeplex-linux
	$(MAKE) install-data

# Needs flatpak-builder (or the org.flatpak.Builder Flatpak); the build fetches crates over the
# network. ponytail: Flathub builds offline, add a generated cargo-sources.json when submitting.
FLATPAK_BUILDER ?= $(shell command -v flatpak-builder || echo flatpak run org.flatpak.Builder)
flatpak:
	$(FLATPAK_BUILDER) --user --install --force-clean --install-deps-from=flathub \
	  --state-dir=../../target/flatpak-builder ../../target/flatpak flatpak/$(ID).json

uninstall:
	rm -f $(BINDIR)/memeplex-linux $(DATADIR)/applications/$(ID).desktop \
	  $(DATADIR)/metainfo/$(ID).metainfo.xml $(ICONS)/scalable/apps/$(ID).svg \
	  $(LOCALE)/*/LC_MESSAGES/memeplex.mo

.PHONY: install install-data stage flatpak uninstall
