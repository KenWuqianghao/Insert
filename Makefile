APP_NAME := Insert
BUNDLE_ID := com.local.Insert
BUILD_DIR := build
APP_BUNDLE := $(BUILD_DIR)/$(APP_NAME).app
DMG_NAME := $(APP_NAME)-Installer.dmg
DMG_PATH := $(BUILD_DIR)/$(DMG_NAME)
DMG_STAGING := $(BUILD_DIR)/dmg
MACOS_DIR := $(APP_BUNDLE)/Contents/MacOS
RESOURCES_DIR := $(APP_BUNDLE)/Contents/Resources
APP_ICON := Resources/AppIcon.icns
ARCH := $(shell uname -m)
SOURCES := $(shell find Sources/Insert -name '*.swift' | sort)
SIGN_IDENTITY ?= -

.PHONY: build run sign dmg clean install marketing-assets

build: $(APP_BUNDLE)

$(APP_ICON): Tools/GenerateIcon.swift
	swift Tools/GenerateIcon.swift

$(APP_BUNDLE): $(SOURCES) Info.plist $(APP_ICON)
	@mkdir -p "$(MACOS_DIR)" "$(RESOURCES_DIR)"
	swiftc -O -parse-as-library -target $(ARCH)-apple-macosx14.0 \
		$(SOURCES) \
		-o "$(MACOS_DIR)/$(APP_NAME)" \
		-framework AppKit \
		-framework SwiftUI \
		-framework Carbon \
		-framework ServiceManagement
	@cp Info.plist "$(APP_BUNDLE)/Contents/Info.plist"
	@cp "$(APP_ICON)" "$(RESOURCES_DIR)/AppIcon.icns"
	@touch "$(APP_BUNDLE)"

run: build
	open -n "$(APP_BUNDLE)"

sign: build
	xattr -cr "$(APP_BUNDLE)"
	codesign --force --deep --options runtime --sign "$(SIGN_IDENTITY)" "$(APP_BUNDLE)"
	codesign --verify --deep --strict --verbose=2 "$(APP_BUNDLE)"

dmg: sign
	@rm -rf "$(DMG_STAGING)" "$(DMG_PATH)"
	@mkdir -p "$(DMG_STAGING)"
	@cp -R "$(APP_BUNDLE)" "$(DMG_STAGING)/"
	@xattr -cr "$(DMG_STAGING)/$(APP_NAME).app"
	@ln -s /Applications "$(DMG_STAGING)/Applications"
	hdiutil create -volname "$(APP_NAME)" -srcfolder "$(DMG_STAGING)" -ov -format UDZO "$(DMG_PATH)"
	@xattr -cr "$(DMG_PATH)"
	hdiutil verify "$(DMG_PATH)"
	@echo "Created $(DMG_PATH)"

marketing-assets:
	@mkdir -p "$(BUILD_DIR)/ModuleCache"
	CLANG_MODULE_CACHE_PATH="$(BUILD_DIR)/ModuleCache" swift Tools/GenerateMarketingAssets.swift

install: build
	@rm -rf "/Applications/$(APP_NAME).app"
	@cp -R "$(APP_BUNDLE)" /Applications/
	@echo "Installed /Applications/$(APP_NAME).app"

clean:
	rm -rf "$(BUILD_DIR)"
