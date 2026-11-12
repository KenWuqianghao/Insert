use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::format::{format_bytes, format_count, grapheme_count, relative_time};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipId(pub Uuid);

impl ClipId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ClipId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ClipId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PinboardId(pub Uuid);

impl PinboardId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PinboardId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub String);

/// `id` is the desktop file id when known, else the window resource class. Compare ids with
/// `app_id_key`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceApp {
    pub id: String,
    pub name: String,
}

/// The form of an app id that is equal for all ids of one app. A window class and a desktop
/// file id can differ in letter case, for example "konsole" and "org.kde.Konsole".
pub fn app_id_key(id: &str) -> String {
    id.to_ascii_lowercase()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

/// Insert decodes only images within these limits. A small file can declare a size that needs
/// gigabytes of memory to decode.
pub const MAX_DECODED_SIDE: u32 = 16_384;
pub const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;

impl PixelSize {
    pub fn can_decode(self) -> bool {
        self.width <= MAX_DECODED_SIDE
            && self.height <= MAX_DECODED_SIDE
            && u64::from(self.width) * u64::from(self.height) * 4 <= MAX_DECODED_BYTES
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorValue {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl ColorValue {
    /// Accepts `#RGB`, `#RRGGBB`, and `#RRGGBBAA`.
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let digits = match digits.len() {
            3 => digits
                .chars()
                .flat_map(|c| [c, c])
                .chain("FF".chars())
                .collect(),
            6 => format!("{digits}FF"),
            8 => digits.to_owned(),
            _ => return None,
        };
        let rgba = u32::from_str_radix(&digits, 16).ok()?;
        let channel = |shift: u32| f64::from((rgba >> shift) & 0xFF) / 255.0;
        Some(Self {
            red: channel(24),
            green: channel(16),
            blue: channel(8),
            alpha: channel(0),
        })
    }

    pub fn rgb8(&self) -> [u8; 3] {
        [self.red, self.green, self.blue].map(to_u8)
    }

    /// `#RRGGBB`, plus `AA` when the color is not opaque.
    pub fn hex(&self) -> String {
        let [r, g, b] = self.rgb8();
        let rgb = format!("#{r:02X}{g:02X}{b:02X}");
        if self.alpha < 0.999 {
            format!("{rgb}{:02X}", to_u8(self.alpha))
        } else {
            rgb
        }
    }

    pub fn is_light(&self) -> bool {
        0.299 * self.red + 0.587 * self.green + 0.114 * self.blue > 0.62
    }
}

fn to_u8(channel: f64) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The index keeps the first `LIMIT` grapheme clusters. The payload keeps the full text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSnippet {
    pub text: String,
    pub character_count: usize,
}

impl TextSnippet {
    pub const LIMIT: usize = 8_000;

    pub fn new(full_text: &str) -> Self {
        let mut count = 0;
        let mut end = full_text.len();
        for (index, _) in full_text.grapheme_indices(true) {
            if count == Self::LIMIT {
                end = index;
            }
            count += 1;
        }
        Self {
            text: full_text[..end].to_owned(),
            character_count: count,
        }
    }

    pub fn first_line(&self) -> Option<&str> {
        self.text
            .split(is_newline)
            .map(|line| line.trim_matches(|c: char| c.is_whitespace() && !is_newline(c)))
            .find(|line| !line.is_empty())
    }
}

fn is_newline(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipKind {
    Text,
    RichText,
    Link,
    Image,
    Files,
    Color,
    Pdf,
    Media,
    Data,
}

impl ClipKind {
    pub const ALL: [ClipKind; 9] = [
        Self::Text,
        Self::RichText,
        Self::Link,
        Self::Image,
        Self::Files,
        Self::Color,
        Self::Pdf,
        Self::Media,
        Self::Data,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::RichText => "Rich Text",
            Self::Link => "Link",
            Self::Image => "Image",
            Self::Files => "Files",
            Self::Color => "Color",
            Self::Pdf => "PDF",
            Self::Media => "Media",
            Self::Data => "Data",
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Text => "format-justify-left-symbolic",
            Self::RichText => "format-text-rich-symbolic",
            Self::Link => "insert-link-symbolic",
            Self::Image => "image-x-generic-symbolic",
            Self::Files => "folder-documents-symbolic",
            Self::Color => "color-select-symbolic",
            Self::Pdf => "x-office-document-symbolic",
            Self::Media => "media-playback-start-symbolic",
            Self::Data => "package-x-generic-symbolic",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipContent {
    Text(TextSnippet),
    RichText(TextSnippet),
    Link {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Image(PixelSize),
    Files(Vec<String>),
    Color(ColorValue),
    Pdf,
    Media,
    Data,
}

impl ClipContent {
    pub fn kind(&self) -> ClipKind {
        match self {
            Self::Text(_) => ClipKind::Text,
            Self::RichText(_) => ClipKind::RichText,
            Self::Link { .. } => ClipKind::Link,
            Self::Image(_) => ClipKind::Image,
            Self::Files(_) => ClipKind::Files,
            Self::Color(_) => ClipKind::Color,
            Self::Pdf => ClipKind::Pdf,
            Self::Media => ClipKind::Media,
            Self::Data => ClipKind::Data,
        }
    }

    pub fn display_title(&self) -> String {
        match self {
            Self::Text(snippet) | Self::RichText(snippet) => snippet
                .first_line()
                .unwrap_or(self.kind().name())
                .to_owned(),
            Self::Link { url, title } => title
                .clone()
                .or_else(|| url_host(url))
                .unwrap_or_else(|| url.clone()),
            Self::Files(uris) => match uris.as_slice() {
                [uri] => file_name(uri),
                _ => format!("{} Files", uris.len()),
            },
            Self::Color(color) => color.hex(),
            Self::Image(_) => "Image".to_owned(),
            Self::Pdf => "PDF Document".to_owned(),
            Self::Media => "Media".to_owned(),
            Self::Data => "Clipboard Data".to_owned(),
        }
    }

    pub fn search_text(&self) -> String {
        match self {
            Self::Text(snippet) | Self::RichText(snippet) => snippet.text.clone(),
            Self::Link {
                url,
                title: Some(title),
            } => format!("{title} {url}"),
            Self::Link { url, title: None } => url.clone(),
            Self::Files(uris) => uris
                .iter()
                .map(|uri| file_path(uri))
                .collect::<Vec<_>>()
                .join(" "),
            Self::Color(color) => color.hex(),
            Self::Image(_) | Self::Pdf | Self::Media | Self::Data => self.kind().name().to_owned(),
        }
    }

    pub fn footer(&self, byte_count: u64) -> String {
        match self {
            Self::Text(snippet) | Self::RichText(snippet) => {
                counted(snippet.character_count, "character")
            }
            Self::Link { url, .. } => counted(grapheme_count(url), "character"),
            Self::Image(size) => format!(
                "{} \u{00D7} {}",
                format_count(size.width as usize),
                format_count(size.height as usize)
            ),
            Self::Files(uris) => counted(uris.len(), "file"),
            Self::Color(color) => {
                let [r, g, b] = color.rgb8();
                format!("RGB {r}, {g}, {b}")
            }
            Self::Pdf | Self::Media | Self::Data => format_bytes(byte_count),
        }
    }
}

fn counted(count: usize, noun: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("{} {noun}{plural}", format_count(count))
}

fn url_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
}

/// The decoded path of a `file://` URI, or the URI itself if it does not parse.
pub fn file_path(uri: &str) -> String {
    url::Url::parse(uri)
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.to_owned())
}

pub fn file_name(uri: &str) -> String {
    let path = file_path(uri);
    let trimmed = path.trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(trimmed)
        .to_owned()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: ClipId,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceApp>,
    pub content: ClipContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinboard_id: Option<PinboardId>,
    pub content_hash: ContentHash,
    pub byte_count: u64,
}

impl Clip {
    pub fn kind(&self) -> ClipKind {
        self.content.kind()
    }

    pub fn footer(&self) -> String {
        self.content.footer(self.byte_count)
    }

    /// `query` must be trimmed and not empty.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        let contains = |text: &str| text.to_lowercase().contains(&query);
        contains(&self.content.search_text())
            || self.custom_title.as_deref().is_some_and(contains)
            || self
                .source
                .as_ref()
                .is_some_and(|source| contains(&source.name))
            || contains(self.kind().name())
    }

    pub fn card_title(&self) -> &str {
        self.custom_title.as_deref().unwrap_or(self.kind().name())
    }

    pub fn card_subtitle(&self, now: OffsetDateTime) -> String {
        let time = relative_time(self.created_at, now);
        match self.custom_title {
            Some(_) => format!("{} \u{00B7} {time}", self.kind().name()),
            None => time,
        }
    }

    pub fn preview_title(&self) -> String {
        self.custom_title
            .clone()
            .unwrap_or_else(|| self.content.display_title())
    }

    pub fn preview_subtitle(&self) -> String {
        let mut parts = vec![self.kind().name().to_owned()];
        if let Some(source) = &self.source {
            parts.push(source.name.clone());
        }
        parts.push(self.footer());
        parts.join(" \u{00B7} ")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinboardColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    const fn hex(value: u32) -> Self {
        Self {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
        }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl PinboardColor {
    pub const ALL: [PinboardColor; 8] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Purple,
        Self::Pink,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Teal => "Teal",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
        }
    }

    pub fn rgb(self, dark: bool) -> Rgb {
        let (light, dark_value) = match self {
            Self::Red => (0xFF3B30, 0xFF453A),
            Self::Orange => (0xFF9500, 0xFF9F0A),
            Self::Yellow => (0xFFCC00, 0xFFD60A),
            Self::Green => (0x34C759, 0x30D158),
            Self::Teal => (0x30B0C7, 0x40C8E0),
            Self::Blue => (0x007AFF, 0x0A84FF),
            Self::Purple => (0xAF52DE, 0xBF5AF2),
            Self::Pink => (0xFF2D55, 0xFF375F),
        };
        Rgb::hex(if dark { dark_value } else { light })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pinboard {
    pub id: PinboardId,
    pub name: String,
    pub color: PinboardColor,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parse_accepts_3_6_8_digits_and_rejects_others() {
        let short = ColorValue::from_hex("#f80").unwrap();
        assert_eq!(short.hex(), "#FF8800");
        assert_eq!(ColorValue::from_hex("#336699").unwrap().alpha, 1.0);
        assert_eq!(
            ColorValue::from_hex("#33669980").unwrap().hex(),
            "#33669980"
        );
        for bad in ["336699", "#3369", "#33669", "#GG0000", "#"] {
            assert_eq!(ColorValue::from_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn is_light_uses_luma_threshold() {
        assert!(ColorValue::from_hex("#FFFFFF").unwrap().is_light());
        assert!(!ColorValue::from_hex("#000000").unwrap().is_light());
        assert!(!ColorValue::from_hex("#FF0000").unwrap().is_light());
    }

    #[test]
    fn snippet_counts_graphemes_and_keeps_first_8000() {
        let text = "e\u{301}".repeat(8_005);
        let snippet = TextSnippet::new(&text);
        assert_eq!(snippet.character_count, 8_005);
        assert_eq!(snippet.text.graphemes(true).count(), 8_000);
    }

    #[test]
    fn first_line_skips_blank_lines_and_trims_spaces() {
        let snippet = TextSnippet::new("\r\n  \t \n\t hello world \nnext");
        assert_eq!(snippet.first_line(), Some("hello world"));
        assert_eq!(TextSnippet::new(" \n\t").first_line(), None);
    }

    fn clip(content: ClipContent) -> Clip {
        Clip {
            id: ClipId::new(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            source: Some(SourceApp {
                id: "org.kde.kate".into(),
                name: "Kate".into(),
            }),
            content,
            custom_title: None,
            pinboard_id: None,
            content_hash: ContentHash(String::new()),
            byte_count: 345_000,
        }
    }

    #[test]
    fn display_strings_per_kind() {
        let link = ClipContent::Link {
            url: "https://example.com/a".into(),
            title: None,
        };
        assert_eq!(link.display_title(), "example.com");
        assert_eq!(link.footer(0), "21 characters");
        let titled = ClipContent::Link {
            url: "https://example.com".into(),
            title: Some("Ex".into()),
        };
        assert_eq!(titled.display_title(), "Ex");
        assert_eq!(titled.search_text(), "Ex https://example.com");

        let one = ClipContent::Files(vec!["file:///home/a/My%20File.txt".into()]);
        assert_eq!(one.display_title(), "My File.txt");
        assert_eq!(one.search_text(), "/home/a/My File.txt");
        assert_eq!(one.footer(0), "1 file");
        let two = ClipContent::Files(vec!["file:///a".into(), "file:///b/".into()]);
        assert_eq!(two.display_title(), "2 Files");
        assert_eq!(two.footer(0), "2 files");

        let image = ClipContent::Image(PixelSize {
            width: 2_560,
            height: 1_440,
        });
        assert_eq!(image.footer(0), "2,560 \u{00D7} 1,440");
        assert_eq!(image.search_text(), "Image");

        let color = ClipContent::Color(ColorValue::from_hex("#FF8000").unwrap());
        assert_eq!(color.display_title(), "#FF8000");
        assert_eq!(color.footer(0), "RGB 255, 128, 0");

        assert_eq!(ClipContent::Pdf.display_title(), "PDF Document");
        assert_eq!(ClipContent::Pdf.footer(345_000), "345 KB");
        assert_eq!(ClipContent::Data.display_title(), "Clipboard Data");

        let text = ClipContent::Text(TextSnippet::new("x"));
        assert_eq!(text.footer(0), "1 character");
        assert_eq!(
            ClipContent::Text(TextSnippet::new("  \n")).display_title(),
            "Text"
        );
    }

    #[test]
    fn matches_content_title_source_and_kind_case_insensitively() {
        let mut clip = clip(ClipContent::Text(TextSnippet::new("Hello World")));
        assert!(clip.matches("world"));
        assert!(clip.matches("KATE"));
        assert!(clip.matches("text"));
        assert!(!clip.matches("greeting"));
        clip.custom_title = Some("Greeting".into());
        assert!(clip.matches("greet"));
        assert!(!clip.matches("wörld"));
    }

    #[test]
    fn card_and_preview_strings() {
        let mut clip = clip(ClipContent::Pdf);
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(5);
        assert_eq!(clip.card_title(), "PDF");
        assert_eq!(clip.card_subtitle(now), "5 min ago");
        assert_eq!(clip.preview_subtitle(), "PDF \u{00B7} Kate \u{00B7} 345 KB");
        clip.custom_title = Some("Invoice".into());
        assert_eq!(clip.card_title(), "Invoice");
        assert_eq!(clip.card_subtitle(now), "PDF \u{00B7} 5 min ago");
        assert_eq!(clip.preview_title(), "Invoice");
    }

    #[test]
    fn pinboard_colors_have_names_and_rgb() {
        assert_eq!(PinboardColor::Teal.name(), "Teal");
        assert_eq!(PinboardColor::Blue.rgb(false).to_hex(), "#007AFF");
        assert_eq!(PinboardColor::Blue.rgb(true).to_hex(), "#0A84FF");
    }

    #[test]
    fn app_id_keys_ignore_letter_case() {
        assert_eq!(app_id_key("org.keepassxc.KeePassXC"), app_id_key("org.keepassxc.keepassxc"));
        assert_ne!(app_id_key("com.bitwarden.desktop"), app_id_key("com.bitwarden"));
    }
}
