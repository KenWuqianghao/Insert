#!/bin/bash
# Usage: Tools/release.sh VERSION [--dry-run]
#
# Sets the version in Info.plist, builds the DMG, signs it for Sparkle, publishes the GitHub release
# on KenWuqianghao/insert-releases, and writes the appcast to $APPCAST.
# --dry-run does all of this except the release and the appcast write. It prints the appcast and
# puts Info.plist back as it was.
set -euo pipefail

RELEASES_REPO="KenWuqianghao/insert-releases"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLIST="$ROOT/Info.plist"
DMG="$ROOT/build/Insert-Installer.dmg"
APPCAST="${APPCAST:-$ROOT/../insert-site/appcast.xml}"

VERSION="${1:-}"
DRY_RUN=false
[[ "${2:-}" == "--dry-run" ]] && DRY_RUN=true

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || [[ $# -gt 2 ]] || [[ $# -eq 2 && "$DRY_RUN" == false ]]; then
	echo "Usage: Tools/release.sh VERSION [--dry-run]   (for example 1.2.0)" >&2
	exit 2
fi

TAG="v$VERSION"
DOWNLOAD_URL="https://github.com/$RELEASES_REPO/releases/download/$TAG/Insert-Installer.dmg"

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
SPARKLE_BIN="$ROOT/vendor/Sparkle-$(awk '/^SPARKLE_VERSION :=/ {print $3}' "$ROOT/Makefile")/bin"

# sign_update reads the EdDSA private key from the login Keychain and prints: sparkle:edSignature="…" length="…"
ENCLOSURE_ATTRIBUTES=$("$SPARKLE_BIN/sign_update" "$DMG")
SIGNATURE=$(sed -E 's/.*sparkle:edSignature="([^"]+)".*/\1/' <<<"$ENCLOSURE_ATTRIBUTES")
"$SPARKLE_BIN/sign_update" --verify "$DMG" "$SIGNATURE"

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

gh release create "$TAG" "$DMG" \
	--repo "$RELEASES_REPO" \
	--title "Insert $VERSION" \
	--notes "Insert $VERSION. Download Insert-Installer.dmg, open it, and drag Insert into Applications."
echo "$APPCAST_XML" > "$APPCAST"

echo "Released $TAG (build $BUILD). Next:"
echo "1. Commit Info.plist in this repo."
echo "2. Commit and deploy $APPCAST in insert-site."
