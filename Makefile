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
FRAMEWORKS_DIR := $(APP_BUNDLE)/Contents/Frameworks
APP_ICON := Resources/AppIcon.icns
ARCH := $(shell uname -m)
SOURCES := $(shell find Sources/Insert -name '*.swift' | sort)
# A certificate gives every build the same designated requirement, so the Accessibility permission stays valid after a rebuild.
# Without one, the build is ad-hoc signed and macOS asks for the permission again after each build.
SIGN_IDENTITY ?= $(or $(shell security find-identity -v -p codesigning 2>/dev/null | awk -F'"' 'NR==1 && NF>2 {print $$2}'),-)
# The hardened runtime rejects an ad-hoc signed Sparkle.framework, so an ad-hoc build does not use it.
CODESIGN := codesign --force $(if $(filter -,$(SIGN_IDENTITY)),,--options runtime) --sign "$(SIGN_IDENTITY)"
NOTARY_PROFILE ?= insert-notary

# The release tarball is not in git. The first build downloads it to vendor/ and checks the hash.
SPARKLE_VERSION := 2.10.1
SPARKLE_SHA256 := 1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
SPARKLE_DIR := vendor/Sparkle-$(SPARKLE_VERSION)
SPARKLE_STAMP := $(SPARKLE_DIR)/.verified
SPARKLE_IN_APP := $(FRAMEWORKS_DIR)/Sparkle.framework

.PHONY: build dev run sign dmg notarize sparkle clean install marketing-assets

# xattr removes Finder data that codesign refuses to sign.
# Sparkle's docs give the signing order: the nested helpers first, then the framework, then the app.
build: $(APP_BUNDLE)
	xattr -cr "$(APP_BUNDLE)"
	$(CODESIGN) "$(SPARKLE_IN_APP)/Versions/B/XPCServices/Installer.xpc"
	$(CODESIGN) --preserve-metadata=entitlements "$(SPARKLE_IN_APP)/Versions/B/XPCServices/Downloader.xpc"
	$(CODESIGN) "$(SPARKLE_IN_APP)/Versions/B/Autoupdate"
	$(CODESIGN) "$(SPARKLE_IN_APP)/Versions/B/Updater.app"
	$(CODESIGN) "$(SPARKLE_IN_APP)"
	$(CODESIGN) "$(APP_BUNDLE)"

sparkle: $(SPARKLE_STAMP)

$(SPARKLE_STAMP):
	@rm -rf "$(SPARKLE_DIR)" && mkdir -p "$(SPARKLE_DIR)"
	curl -fsSL -o "$(SPARKLE_DIR).tar.xz" "https://github.com/sparkle-project/Sparkle/releases/download/$(SPARKLE_VERSION)/Sparkle-$(SPARKLE_VERSION).tar.xz"
	echo "$(SPARKLE_SHA256)  $(SPARKLE_DIR).tar.xz" | shasum -a 256 -c -
	tar -xJf "$(SPARKLE_DIR).tar.xz" -C "$(SPARKLE_DIR)"
	@rm "$(SPARKLE_DIR).tar.xz"
	@touch "$@"

$(APP_ICON): Tools/GenerateIcon.swift
	swift Tools/GenerateIcon.swift

$(APP_BUNDLE): $(SOURCES) Info.plist $(APP_ICON) $(SPARKLE_STAMP)
	@mkdir -p "$(MACOS_DIR)" "$(RESOURCES_DIR)" "$(FRAMEWORKS_DIR)"
	swiftc -O -parse-as-library -target $(ARCH)-apple-macosx14.0 \
		$(SOURCES) \
		-o "$(MACOS_DIR)/$(APP_NAME)" \
		-F "$(SPARKLE_DIR)" \
		-framework Sparkle \
		-Xlinker -rpath -Xlinker @executable_path/../Frameworks \
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
	@rm -rf "$(SPARKLE_IN_APP)"
	@ditto "$(SPARKLE_DIR)/Sparkle.framework" "$(SPARKLE_IN_APP)"
	@touch "$(APP_BUNDLE)"

# A copy with its own bundle id. Its storage folder and defaults are separate from an installed Insert.
# It does not check for updates automatically, so a release never replaces it.
dev: build
	@rm -rf "$(DEV_BUNDLE)"
	@cp -R "$(APP_BUNDLE)" "$(DEV_BUNDLE)"
	@/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $(BUNDLE_ID).dev" -c "Set :SUEnableAutomaticChecks false" "$(DEV_BUNDLE)/Contents/Info.plist"
	@$(CODESIGN) "$(DEV_BUNDLE)"
	@echo "Created $(DEV_BUNDLE) ($(BUNDLE_ID).dev)"

run: build
	open -n "$(APP_BUNDLE)"

sign: build
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

# This needs a Developer ID certificate and a notarytool profile: xcrun notarytool store-credentials "$(NOTARY_PROFILE)"
# Run it after make dmg and before Sparkle signs the DMG, because stapling changes the file.
notarize:
	xcrun notarytool submit "$(DMG_PATH)" --keychain-profile "$(NOTARY_PROFILE)" --wait
	xcrun stapler staple "$(DMG_PATH)"

# The capture can be a few pixels larger than the 16:10 stage. This crop removes the extra pixels.
# ScreenCaptureKit writes no frames while the stage does not change, so the pad holds the last frame after the paste.
DEMO_FRAME := crop='trunc(min(iw\,ih*16/10)/2)*2':'trunc(min(ih\,iw*10/16)/2)*2',tpad=stop_mode=clone:stop_duration=1.5

# The tool records the real tray views with sample clips. It needs the Screen Recording permission and ffmpeg.
marketing-assets: $(SPARKLE_STAMP)
	@mkdir -p "$(MARKETING_DIR)"
	swiftc -O -parse-as-library -target $(ARCH)-apple-macosx15.0 \
		$(filter-out Sources/Insert/App/InsertApp.swift,$(SOURCES)) Tools/GenerateMarketingAssets.swift \
		-o "$(BUILD_DIR)/MarketingAssets" \
		-F "$(SPARKLE_DIR)" \
		-framework Sparkle \
		-Xlinker -rpath -Xlinker "$(CURDIR)/$(SPARKLE_DIR)" \
		-framework AppKit \
		-framework SwiftUI \
		-framework Carbon \
		-framework ServiceManagement \
		-framework ApplicationServices \
		-framework CryptoKit \
		-framework ImageIO \
		-framework UniformTypeIdentifiers \
		-framework ScreenCaptureKit \
		-framework AVFoundation
	"$(BUILD_DIR)/MarketingAssets" "$(MARKETING_DIR)"
	@cp "$(MARKETING_DIR)"/insert-*.png docs/assets/
	ffmpeg -loglevel error -y -i "$(MARKETING_DIR)/insert-demo-raw.mp4" \
		-vf "$(DEMO_FRAME),fps=60,scale=1920:1200:flags=lanczos" -c:v libx264 -crf 23 -preset slow -pix_fmt yuv420p -movflags +faststart -an \
		docs/assets/insert-demo.mp4
	ffmpeg -loglevel error -y -i "$(MARKETING_DIR)/insert-demo-raw.mp4" \
		-vf "$(DEMO_FRAME),fps=12,scale=960:600:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
		docs/assets/insert-demo.gif
	@echo "Updated docs/assets. The step times of the demo are in $(MARKETING_DIR)/insert-demo-chapters.json"

install: build
	@rm -rf "/Applications/$(APP_NAME).app"
	@cp -R "$(APP_BUNDLE)" /Applications/
	@echo "Installed /Applications/$(APP_NAME).app"

clean:
	rm -rf "$(BUILD_DIR)"
