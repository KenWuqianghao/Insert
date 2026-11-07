#!/bin/bash
# Usage: Tools/release.sh VERSION [--dry-run]
#
# Sets the version in Info.plist, builds the DMG, signs it for Sparkle, publishes the GitHub release
# on KenWuqianghao/Insert, and writes the appcast to $APPCAST.
# --dry-run does all of this except the release and the appcast write. It prints the appcast and
# puts Info.plist back as it was.
set -euo pipefail

RELEASES_REPO="KenWuqianghao/Insert"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLIST="$ROOT/Info.plist"
DMG="$ROOT/build/Insert-Installer.dmg"
# Sparkle downloads this copy of the DMG, so its GitHub download count is the update count.
UPDATE_DMG="$ROOT/build/Insert-Update.dmg"
APPCAST="${APPCAST:-$ROOT/../insert-site/appcast.xml}"

VERSION="${1:-}"
DRY_RUN=false
[[ "${2:-}" == "--dry-run" ]] && DRY_RUN=true

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || [[ $# -gt 2 ]] || [[ $# -eq 2 && "$DRY_RUN" == false ]]; then
	echo "Usage: Tools/release.sh VERSION [--dry-run]   (for example 1.2.0)" >&2
	exit 2
fi

TAG="v$VERSION"
DOWNLOAD_URL="https://github.com/$RELEASES_REPO/releases/download/$TAG/Insert-Update.dmg"

if ! $DRY_RUN; then
	[[ -f "$APPCAST" ]] || { echo "No appcast at $APPCAST. Set APPCAST to the insert-site appcast.xml." >&2; exit 1; }
	gh repo view "$RELEASES_REPO" >/dev/null
	if gh release view "$TAG" --repo "$RELEASES_REPO" >/dev/null 2>&1; then
		echo "$RELEASES_REPO already has the release $TAG." >&2
		exit 1
	fi
fi

if $DRY_RUN; then
	PLIST_BACKUP="$(mktemp)"
	cp "$PLIST" "$PLIST_BACKUP"
	trap 'cp "$PLIST_BACKUP" "$PLIST"; rm -f "$PLIST_BACKUP"' EXIT
fi

BUILD=$(( $(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$PLIST") + 1 ))
/usr/libexec/PlistBuddy \
	-c "Set :CFBundleShortVersionString $VERSION" \
	-c "Set :CFBundleVersion $BUILD" \
	"$PLIST"
MINIMUM_SYSTEM=$(/usr/libexec/PlistBuddy -c "Print :LSMinimumSystemVersion" "$PLIST")

make -C "$ROOT" dmg
cp "$DMG" "$UPDATE_DMG"
SPARKLE_BIN="$ROOT/vendor/Sparkle-$(awk '/^SPARKLE_VERSION :=/ {print $3}' "$ROOT/Makefile")/bin"
APP_PLIST="$ROOT/build/Insert.app/Contents/Info.plist"

# Installed copies accept only updates signed with the key in SUPublicEDKey.
KEYCHAIN_KEY=$("$SPARKLE_BIN/generate_keys" -p)
APP_KEY=$(/usr/libexec/PlistBuddy -c "Print :SUPublicEDKey" "$APP_PLIST")
if [[ "$KEYCHAIN_KEY" != "$APP_KEY" ]]; then
	echo "The Sparkle key in the Keychain ($KEYCHAIN_KEY) is not SUPublicEDKey ($APP_KEY). Restore the Keychain key before you release." >&2
	exit 1
fi

# The Makefile builds for the host CPU only, so the appcast offers the update only to that CPU.
ARCHS=$(lipo -archs "$ROOT/build/Insert.app/Contents/MacOS/Insert")
[[ "$ARCHS" == "arm64" || "$ARCHS" == "x86_64" ]] || { echo "Unexpected architectures: $ARCHS" >&2; exit 1; }

# sign_update reads the EdDSA private key from the login Keychain and prints: sparkle:edSignature="…" length="…"
ENCLOSURE_ATTRIBUTES=$("$SPARKLE_BIN/sign_update" "$UPDATE_DMG")
SIGNATURE=$(sed -E 's/.*sparkle:edSignature="([^"]+)".*/\1/' <<<"$ENCLOSURE_ATTRIBUTES")
"$SPARKLE_BIN/sign_update" --verify "$UPDATE_DMG" "$SIGNATURE"

APPCAST_XML=$(cat <<EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Insert</title>
    <link>https://insert-app.vercel.app/</link>
    <description>Updates for Insert</description>
    <language>en</language>
    <item>
      <title>Insert $VERSION</title>
      <pubDate>$(LC_ALL=C date -u "+%a, %d %b %Y %H:%M:%S +0000")</pubDate>
      <sparkle:version>$BUILD</sparkle:version>
      <sparkle:shortVersionString>$VERSION</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>$MINIMUM_SYSTEM</sparkle:minimumSystemVersion>
      <sparkle:hardwareRequirements>$ARCHS</sparkle:hardwareRequirements>
      <enclosure url="$DOWNLOAD_URL" type="application/octet-stream" $ENCLOSURE_ATTRIBUTES/>
    </item>
  </channel>
</rss>
EOF
)
xmllint --noout - <<<"$APPCAST_XML"

if $DRY_RUN; then
	echo "Dry run: skipped gh release create $TAG on $RELEASES_REPO. The appcast for $APPCAST:"
	echo "$APPCAST_XML"
	exit 0
fi

# The appcast is written next to its target first, so the release step is the last one that can fail.
APPCAST_NEXT="$(mktemp "$APPCAST.XXXXXX")"
echo "$APPCAST_XML" > "$APPCAST_NEXT"
trap 'rm -f "$APPCAST_NEXT"' EXIT

gh release create "$TAG" "$DMG" "$UPDATE_DMG" \
	--repo "$RELEASES_REPO" \
	--title "Insert $VERSION" \
	--notes "Insert $VERSION. Download Insert-Installer.dmg, open it, and drag Insert into Applications. Insert-Update.dmg is the same file for automatic updates."
mv "$APPCAST_NEXT" "$APPCAST"

echo "Released $TAG (build $BUILD). Next:"
echo "1. Commit Info.plist in this repo."
echo "2. Commit and deploy $APPCAST in insert-site."
