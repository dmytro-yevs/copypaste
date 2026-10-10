PKG_CONFIG ?= pkg-config
MUTTER_PKG ?= libmutter-14
BUILD := build
LIB := $(BUILD)/libcopypaste_clipboard_source.so
GIR := $(BUILD)/CopyPasteClipboard-1.0.gir
TYPELIB := $(BUILD)/CopyPasteClipboard-1.0.typelib
CFLAGS := -fPIC -O2 -Wall -Wextra -Werror $(shell $(PKG_CONFIG) --cflags gio-2.0 gobject-2.0 gmodule-2.0 $(MUTTER_PKG))
LIBS := $(shell $(PKG_CONFIG) --libs gio-2.0 gobject-2.0 gmodule-2.0)

all: $(LIB) $(TYPELIB)

$(BUILD):
	mkdir -p $@

$(LIB): copypaste-clipboard-source.c copypaste-clipboard-source.h | $(BUILD)
	$(CC) $(CFLAGS) -shared -o $@ copypaste-clipboard-source.c $(LIBS)

$(GIR): copypaste-clipboard-source.c copypaste-clipboard-source.h $(LIB)
	g-ir-scanner --namespace=CopyPasteClipboard --nsversion=1.0 --identifier-prefix=CopyPaste --symbol-prefix=copypaste --include=GObject-2.0 --include=Gio-2.0 --cflags-begin $(CFLAGS) --cflags-end --library=copypaste_clipboard_source --library-path=$(BUILD) --output=$@ copypaste-clipboard-source.c copypaste-clipboard-source.h

$(TYPELIB): $(GIR)
	g-ir-compiler $< -o $@

clean:
	rm -rf $(BUILD)
