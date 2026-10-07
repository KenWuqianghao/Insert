APP_NAME := Insert
BUNDLE_ID := com.local.Insert
BUILD_DIR := build
APP_BUNDLE := $(BUILD_DIR)/$(APP_NAME).app
DEV_BUNDLE := $(BUILD_DIR)/$(APP_NAME)Dev.app
DMG_NAME := $(APP_NAME)-Installer.dmg
DMG_PATH := $(BUILD_DIR)/$(DMG_NAME)
DMG_STAGING := $(BUILD_DIR)/dmg
MARKETING_DIR := $(BUILD_DIR)/marketing
MACOS_DIR := $(APP_BUNDLE)/Contents/MacOS
RESOURCES_DIR := $(APP_BUNDLE)/Contents/Resources
APP_ICON := Resources/AppIcon.icns
ARCH := $(shell uname -m)
SOURCES := $(shell find Sources/Insert -name '*.swift' | sort)
SIGN_IDENTITY ?= -

.PHONY: build dev run sign dmg clean install marketing-assets

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
		-framework ServiceManagement \
		-framework ApplicationServices \
		-framework CryptoKit \
		-framework ImageIO \
		-framework UniformTypeIdentifiers
	@cp Info.plist "$(APP_BUNDLE)/Contents/Info.plist"
	@cp "$(APP_ICON)" "$(RESOURCES_DIR)/AppIcon.icns"
	@touch "$(APP_BUNDLE)"

# A copy with its own bundle id. Its storage folder and defaults are separate from an installed Insert.
dev: build
	@rm -rf "$(DEV_BUNDLE)"
	@cp -R "$(APP_BUNDLE)" "$(DEV_BUNDLE)"
	@/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $(BUNDLE_ID).dev" "$(DEV_BUNDLE)/Contents/Info.plist"
	@codesign --force --sign - "$(DEV_BUNDLE)"
	@echo "Created $(DEV_BUNDLE) ($(BUNDLE_ID).dev)"

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

# The tool records the real tray views with sample clips. It needs the Screen Recording permission and ffmpeg.
marketing-assets:
	@mkdir -p "$(MARKETING_DIR)"
	swiftc -O -parse-as-library -target $(ARCH)-apple-macosx15.0 \
		$(filter-out Sources/Insert/App/InsertApp.swift,$(SOURCES)) Tools/GenerateMarketingAssets.swift \
		-o "$(BUILD_DIR)/MarketingAssets" \
		-framework AppKit \
		-framework SwiftUI \
		-framework Carbon \
		-framework ServiceManagement \
		-framework ApplicationServices \
		-framework CryptoKit \
		-framework ImageIO \
		-framework UniformTypeIdentifiers \
		-framework ScreenCaptureKit
	"$(BUILD_DIR)/MarketingAssets" "$(MARKETING_DIR)"
	@cp "$(MARKETING_DIR)"/insert-*.png docs/assets/
	ffmpeg -loglevel error -y -i "$(MARKETING_DIR)/insert-demo-raw.mp4" \
		-vf "fps=30,scale=1920:-2:flags=lanczos" -c:v libx264 -crf 23 -preset slow -pix_fmt yuv420p -movflags +faststart -an \
		docs/assets/insert-demo.mp4
	ffmpeg -loglevel error -y -i "$(MARKETING_DIR)/insert-demo-raw.mp4" \
		-vf "fps=12,scale=960:-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
		docs/assets/insert-demo.gif
	@echo "Updated docs/assets"

install: build
	@rm -rf "/Applications/$(APP_NAME).app"
	@cp -R "$(APP_BUNDLE)" /Applications/
	@echo "Installed /Applications/$(APP_NAME).app"

clean:
	rm -rf "$(BUILD_DIR)"
