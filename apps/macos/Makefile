# Building CopyPaste.app from the Swift package.
#
# NOTHING IN THIS FILE HAS EVER BEEN RUN. It was written on a Linux host with
# no Swift toolchain and no macOS SDK; treat every rule as a proposal to check,
# not as a tested build. See README.md, "What has never been executed".
#
# `swift build` alone produces a bare executable, which is enough to run the app
# but not enough for the two things that need a bundle: LSUIElement (no Dock
# icon) and SMAppService (open at login). Hence the `app` rule.

APP_NAME    := CopyPaste
BUILD_DIR   := .build
CONFIG      := release
BIN         := $(BUILD_DIR)/$(CONFIG)/$(APP_NAME)
APP         := $(BUILD_DIR)/$(APP_NAME).app
CONTENTS    := $(APP)/Contents
# Ad-hoc signature: there is no Developer ID for this project. See README.md —
# this is exactly why the app must not require any TCC permission.
CODESIGN_ID := -

.PHONY: all build app run clean test

all: app

build:
	swift build -c $(CONFIG)

test:
	swift test

app: build
	rm -rf "$(APP)"
	mkdir -p "$(CONTENTS)/MacOS" "$(CONTENTS)/Resources"
	cp "$(BIN)" "$(CONTENTS)/MacOS/$(APP_NAME)"
	cp App/Info.plist "$(CONTENTS)/Info.plist"
	printf 'APPL????' > "$(CONTENTS)/PkgInfo"
	# The service binary rides along when it has been built, so the app can
	# start its own rather than hunting for one on PATH.
	@if [ -f ../../target/release/copypaste-daemon ]; then \
		cp ../../target/release/copypaste-daemon "$(CONTENTS)/MacOS/copypaste-daemon"; \
		echo "bundled copypaste-daemon"; \
	else \
		echo "no target/release/copypaste-daemon — the app will look for one on disk"; \
	fi
	codesign --force --sign "$(CODESIGN_ID)" \
		--entitlements App/CopyPaste.entitlements \
		--options runtime \
		"$(APP)"
	@echo "built $(APP)"

run: app
	open "$(APP)"

clean:
	rm -rf "$(BUILD_DIR)"
