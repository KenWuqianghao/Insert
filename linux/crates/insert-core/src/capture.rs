use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use sha2::{Digest, Sha256};

use crate::mime::{
    image_rank, is_media, is_plain_stored, is_privacy_marker, is_rich_text, is_volatile,
    text_source, MimeMap, TextEncoding, COLOR, GNOME_COPIED_FILES, HTML, KDE_PASSWORD_HINT, PDF,
    UTF8_TEXT,
};
use crate::model::{
    ClipContent, ColorValue, ContentHash, PixelSize, TextSnippet, MAX_DECODED_BYTES,
    MAX_DECODED_SIDE,
};

pub const MAX_CAPTURE_BYTES: u64 = 50 * 1024 * 1024;
pub const THUMBNAIL_MAX_SIDE: u32 = 560;

#[derive(Clone, Debug)]
pub struct Capture {
    pub content: ClipContent,
    /// Sorted by MIME name.
    pub payload: MimeMap,
    pub thumbnail_png: Option<Vec<u8>>,
    pub content_hash: ContentHash,
    pub byte_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    Concealed,
    TooLarge,
    NothingStored,
    Blank,
}

pub fn classify(offer: MimeMap) -> Result<Capture, Skip> {
    if is_concealed(&offer) {
        return Err(Skip::Concealed);
    }
    let payload = stored_payload(offer)?;
    let (content, thumbnail_png) = classify_payload(&payload)?;
    Ok(Capture {
        content,
        content_hash: content_hash(&payload),
        byte_count: payload.byte_count(),
        payload,
        thumbnail_png,
    })
}

fn is_concealed(offer: &MimeMap) -> bool {
    offer.0.iter().any(|(mime, data)| {
        is_privacy_marker(mime) || (mime == KDE_PASSWORD_HINT && data.trim_ascii() == b"secret")
    })
}

fn stored_payload(offer: MimeMap) -> Result<MimeMap, Skip> {
    let mut text: Option<(usize, String)> = None;
    let mut image: Option<(usize, String, Vec<u8>)> = None;
    let mut entries = Vec::new();

    for (mime, data) in offer.0 {
        if data.is_empty() {
            continue;
        }
        if let Some((rank, encoding)) = text_source(&mime) {
            if text.as_ref().is_none_or(|(best, _)| rank < *best) {
                if let Some(decoded) = decode_text(&data, encoding) {
                    text = Some((rank, decoded));
                }
            }
        } else if let Some(rank) = image_rank(&mime) {
            if image.as_ref().is_none_or(|(best, _, _)| rank < *best) {
                image = Some((rank, mime, data));
            }
        } else if mime == GNOME_COPIED_FILES {
            entries.push((mime, as_copy_operation(data)));
        } else if is_plain_stored(&mime) {
            entries.push((mime, data));
        }
    }

    entries.extend(text.map(|(_, text)| (UTF8_TEXT.to_owned(), text.into_bytes())));
    entries.extend(image.map(|(_, mime, data)| (mime, data)));
    if entries.is_empty() {
        return Err(Skip::NothingStored);
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let payload = MimeMap(entries);
    if payload.byte_count() > MAX_CAPTURE_BYTES {
        return Err(Skip::TooLarge);
    }
    Ok(payload)
}

fn decode_text(data: &[u8], encoding: TextEncoding) -> Option<String> {
    match encoding {
        TextEncoding::Utf8 => String::from_utf8(data.to_vec()).ok(),
        TextEncoding::Latin1 => Some(data.iter().map(|&byte| char::from(byte)).collect()),
    }
}

/// A stored "cut" would move the files again on each paste from the history.
fn as_copy_operation(data: Vec<u8>) -> Vec<u8> {
    match data.strip_prefix(b"cut") {
        Some(rest)
            if rest
                .first()
                .is_none_or(|byte| *byte == b'\n' || *byte == b'\r') =>
        {
            [b"copy".as_slice(), rest].concat()
        }
        _ => data,
    }
}

/// SHA-256 over the non-volatile entries in MIME name order: name, 0x1E, length as u64 LE, data.
pub fn content_hash(payload: &MimeMap) -> ContentHash {
    let mut entries: Vec<&(String, Vec<u8>)> = payload
        .0
        .iter()
        .filter(|(mime, _)| !is_volatile(mime))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (mime, data) in entries {
        hasher.update(mime.as_bytes());
        hasher.update([0x1E]);
        hasher.update((data.len() as u64).to_le_bytes());
        hasher.update(data);
    }
    ContentHash(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

fn classify_payload(payload: &MimeMap) -> Result<(ClipContent, Option<Vec<u8>>), Skip> {
    let files = payload.file_uris();
    if !files.is_empty() {
        return Ok((ClipContent::Files(files), None));
    }
    if let Some(data) = payload.image() {
        if let Some(size) = image_size(data) {
            let png = size.can_decode().then(|| thumbnail(data)).flatten();
            return Ok((ClipContent::Image(size), png));
        }
    }
    if let Some((url, title)) = payload.url_and_title() {
        return Ok((ClipContent::Link { url, title }, None));
    }
    if payload.get(PDF).is_some() {
        return Ok((ClipContent::Pdf, None));
    }
    if let Some(color) = payload.get(COLOR).and_then(gtk_color) {
        return Ok((ClipContent::Color(color), None));
    }

    let text = payload
        .get(UTF8_TEXT)
        .and_then(|data| std::str::from_utf8(data).ok());
    let has_text = text.is_some_and(|text| !text.trim().is_empty());

    if let Some((_, rtf)) = payload.0.iter().find(|(mime, _)| is_rich_text(mime)) {
        let plain = match text {
            Some(text) if has_text => Some(text.to_owned()),
            _ => Some(rtf_to_text(rtf)).filter(|text| !text.trim().is_empty()),
        };
        return plain
            .map(|text| (ClipContent::RichText(TextSnippet::new(&text)), None))
            .ok_or(Skip::Blank);
    }

    if let Some(text) = text.filter(|_| has_text) {
        let trimmed = text.trim();
        if is_web_url(trimmed) {
            return Ok((
                ClipContent::Link {
                    url: trimmed.to_owned(),
                    title: None,
                },
                None,
            ));
        }
        if let Some(color) = ColorValue::from_hex(trimmed) {
            return Ok((ClipContent::Color(color), None));
        }
        return Ok((ClipContent::Text(TextSnippet::new(text)), None));
    }

    if let Some(html) = payload.get(HTML) {
        let stripped = strip_html(&String::from_utf8_lossy(html));
        return match stripped.is_empty() {
            true => Err(Skip::Blank),
            false => Ok((ClipContent::RichText(TextSnippet::new(&stripped)), None)),
        };
    }

    if payload.mimes().any(is_media) {
        return Ok((ClipContent::Media, None));
    }

    match text {
        Some(_) => Err(Skip::Blank),
        None => Ok((ClipContent::Data, None)),
    }
}

fn image_reader(data: &[u8]) -> Option<ImageReader<Cursor<&[u8]>>> {
    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()
}

fn image_size(data: &[u8]) -> Option<PixelSize> {
    let (width, height) = image_reader(data)?.into_dimensions().ok()?;
    (width > 0 && height > 0).then_some(PixelSize { width, height })
}

fn thumbnail(data: &[u8]) -> Option<Vec<u8>> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_SIDE);
    limits.max_image_height = Some(MAX_DECODED_SIDE);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    let mut reader = image_reader(data)?;
    reader.limits(limits);
    let mut decoder = reader.into_decoder().ok()?;
    // The decoder allocates its output buffer without a check against `max_alloc`.
    if decoder.total_bytes() > MAX_DECODED_BYTES {
        return None;
    }
    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    if image.width().max(image.height()) > THUMBNAIL_MAX_SIDE {
        image = image.resize(
            THUMBNAIL_MAX_SIDE,
            THUMBNAIL_MAX_SIDE,
            image::imageops::FilterType::Triangle,
        );
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// GTK writes `application/x-color` as four u16 values: red, green, blue, alpha.
fn gtk_color(data: &[u8]) -> Option<ColorValue> {
    let channels: Vec<f64> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| f64::from(u16::from_le_bytes([pair[0], pair[1]])) / 65_535.0)
        .collect();
    match channels.as_slice() {
        [red, green, blue, alpha] => Some(ColorValue {
            red: *red,
            green: *green,
            blue: *blue,
            alpha: *alpha,
        }),
        _ => None,
    }
}

fn is_web_url(text: &str) -> bool {
    if text.contains(char::is_whitespace) {
        return false;
    }
    url::Url::parse(text).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some_and(|host| !host.is_empty())
    })
}

fn strip_html(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        match rest[start + 1..].find('>') {
            Some(0) => {
                text.push_str("<>");
                rest = &rest[start + 2..];
            }
            Some(length) => {
                text.push(' ');
                rest = &rest[start + 1 + length + 1..];
            }
            None => {
                text.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    text.push_str(rest);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rtf_to_text(rtf: &[u8]) -> String {
    const SKIPPED: [&str; 16] = [
        "fonttbl",
        "colortbl",
        "stylesheet",
        "info",
        "pict",
        "header",
        "footer",
        "object",
        "listtable",
        "listoverridetable",
        "rsidtbl",
        "generator",
        "xmlnstbl",
        "themedata",
        "colorschememapping",
        "latentstyles",
    ];
    let mut text = String::new();
    let mut groups: Vec<(bool, usize)> = Vec::new();
    let (mut skipping, mut unicode_fallback, mut pending_fallback) = (false, 1usize, 0usize);
    let mut push = |c: char, skipping: bool, pending: &mut usize| {
        if *pending > 0 {
            *pending -= 1;
        } else if !skipping {
            text.push(c);
        }
    };
    let mut index = 0;
    while index < rtf.len() {
        let byte = rtf[index];
        index += 1;
        match byte {
            b'{' => groups.push((skipping, unicode_fallback)),
            b'}' => {
                (skipping, unicode_fallback) = groups.pop().unwrap_or((skipping, unicode_fallback))
            }
            b'\r' | b'\n' => {}
            b'\\' => {
                let Some(&next) = rtf.get(index) else { break };
                if next.is_ascii_alphabetic() {
                    let start = index;
                    while rtf.get(index).is_some_and(u8::is_ascii_alphabetic) {
                        index += 1;
                    }
                    let word = std::str::from_utf8(&rtf[start..index]).unwrap_or_default();
                    let number_start = index;
                    if rtf.get(index) == Some(&b'-') {
                        index += 1;
                    }
                    while rtf.get(index).is_some_and(u8::is_ascii_digit) {
                        index += 1;
                    }
                    let parameter: Option<i32> = std::str::from_utf8(&rtf[number_start..index])
                        .ok()
                        .and_then(|n| n.parse().ok());
                    if rtf.get(index) == Some(&b' ') {
                        index += 1;
                    }
                    match word {
                        "par" | "line" => push('\n', skipping, &mut 0),
                        "tab" => push('\t', skipping, &mut 0),
                        "uc" => unicode_fallback = parameter.unwrap_or(1).max(0) as usize,
                        "u" => {
                            let code = parameter.unwrap_or(0);
                            let code = if code < 0 { code + 65_536 } else { code };
                            push(
                                char::from_u32(code as u32).unwrap_or('\u{FFFD}'),
                                skipping,
                                &mut 0,
                            );
                            pending_fallback = unicode_fallback;
                        }
                        _ if SKIPPED.contains(&word) => skipping = true,
                        _ => {}
                    }
                } else {
                    index += 1;
                    match next {
                        b'\'' => {
                            let hex = rtf
                                .get(index..index + 2)
                                .and_then(|h| std::str::from_utf8(h).ok());
                            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                                push(char::from(value), skipping, &mut pending_fallback);
                            }
                            index += 2;
                        }
                        b'*' => skipping = true,
                        b'~' => push('\u{A0}', skipping, &mut pending_fallback),
                        b'\\' | b'{' | b'}' => {
                            push(char::from(next), skipping, &mut pending_fallback)
                        }
                        _ => {}
                    }
                }
            }
            _ => push(char::from(byte), skipping, &mut pending_fallback),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::{MOZ_URL, URI_LIST};
    use crate::model::ClipKind;

    fn offer(entries: &[(&str, &[u8])]) -> MimeMap {
        MimeMap(
            entries
                .iter()
                .map(|(mime, data)| (mime.to_string(), data.to_vec()))
                .collect(),
        )
    }

    fn kind(entries: &[(&str, &[u8])]) -> Result<ClipKind, Skip> {
        classify(offer(entries)).map(|capture| capture.content.kind())
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::new_rgb8(width, height);
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn plain_text_is_text_and_stored_once_as_utf8() {
        let capture = classify(offer(&[
            ("TARGETS", b"x"),
            ("UTF8_STRING", b"hello"),
            ("text/plain", b"hello"),
            ("text/plain;charset=utf-8", b"hello"),
        ]))
        .unwrap();
        assert_eq!(
            capture.content,
            ClipContent::Text(TextSnippet::new("hello"))
        );
        assert_eq!(capture.payload, offer(&[(UTF8_TEXT, b"hello")]));
        assert_eq!(capture.byte_count, 5);
    }

    #[test]
    fn latin1_string_target_is_decoded() {
        let capture = classify(offer(&[("STRING", b"caf\xE9")])).unwrap();
        assert_eq!(capture.content, ClipContent::Text(TextSnippet::new("café")));
    }

    #[test]
    fn web_url_text_is_link_and_hex_text_is_color() {
        let link = classify(offer(&[(UTF8_TEXT, b"  https://example.com/a?b=1\n")])).unwrap();
        assert_eq!(
            link.content,
            ClipContent::Link {
                url: "https://example.com/a?b=1".into(),
                title: None
            }
        );
        assert_eq!(
            kind(&[(UTF8_TEXT, b"https://exa mple.com")]),
            Ok(ClipKind::Text)
        );
        assert_eq!(
            kind(&[(UTF8_TEXT, b"ftp://example.com")]),
            Ok(ClipKind::Text)
        );
        assert_eq!(kind(&[(UTF8_TEXT, b" #ff8800 ")]), Ok(ClipKind::Color));
    }

    #[test]
    fn uri_list_with_files_is_files() {
        let capture = classify(offer(&[
            (
                URI_LIST,
                b"# comment\r\nfile:///home/u/a.txt\r\nfile:///home/u/b%20c.txt\r\n",
            ),
            (UTF8_TEXT, b"/home/u/a.txt"),
        ]))
        .unwrap();
        assert_eq!(
            capture.content,
            ClipContent::Files(vec![
                "file:///home/u/a.txt".into(),
                "file:///home/u/b%20c.txt".into()
            ])
        );
    }

    #[test]
    fn gnome_copied_files_is_files_and_cut_is_stored_as_copy() {
        let capture = classify(offer(&[(GNOME_COPIED_FILES, b"cut\nfile:///tmp/x")])).unwrap();
        assert_eq!(
            capture.content,
            ClipContent::Files(vec!["file:///tmp/x".into()])
        );
        assert_eq!(
            capture.payload.get(GNOME_COPIED_FILES),
            Some(b"copy\nfile:///tmp/x".as_slice())
        );
    }

    #[test]
    fn uri_list_with_web_uri_is_link() {
        assert_eq!(
            classify(offer(&[(URI_LIST, b"https://example.com/\r\n")]))
                .unwrap()
                .content,
            ClipContent::Link {
                url: "https://example.com/".into(),
                title: None
            }
        );
    }

    #[test]
    fn moz_url_gives_link_with_title() {
        let utf16: Vec<u8> = "https://example.com\nExample"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let capture = classify(offer(&[
            (MOZ_URL, &utf16),
            (UTF8_TEXT, b"https://example.com"),
        ]))
        .unwrap();
        assert_eq!(
            capture.content,
            ClipContent::Link {
                url: "https://example.com".into(),
                title: Some("Example".into())
            }
        );
    }

    #[test]
    fn image_gives_size_thumbnail_and_keeps_one_image_entry() {
        let capture = classify(offer(&[
            ("image/bmp", b"not decoded"),
            ("image/png", &png(1_120, 280)),
            ("application/x-qt-image", b"x"),
        ]))
        .unwrap();
        assert_eq!(
            capture.content,
            ClipContent::Image(PixelSize {
                width: 1_120,
                height: 280
            })
        );
        assert_eq!(capture.payload.mimes().collect::<Vec<_>>(), ["image/png"]);
        let thumbnail = image::load_from_memory(&capture.thumbnail_png.unwrap()).unwrap();
        assert_eq!((thumbnail.width(), thumbnail.height()), (560, 140));
    }

    #[test]
    fn small_image_is_not_scaled_up() {
        let capture = classify(offer(&[("image/png", &png(40, 30))])).unwrap();
        let thumbnail = image::load_from_memory(&capture.thumbnail_png.unwrap()).unwrap();
        assert_eq!((thumbnail.width(), thumbnail.height()), (40, 30));
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = !0u32;
            for byte in bytes {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
                }
            }
            !crc
        }
        let chunk = |kind: &[u8], body: &[u8]| {
            let typed = [kind, body].concat();
            [&(body.len() as u32).to_be_bytes()[..], &typed, &crc32(&typed).to_be_bytes()].concat()
        };
        let ihdr = [&width.to_be_bytes()[..], &height.to_be_bytes(), &[8, 6, 0, 0, 0]].concat();
        [
            &b"\x89PNG\r\n\x1a\n"[..],
            &chunk(b"IHDR", &ihdr),
            &chunk(b"IDAT", &[]),
            &chunk(b"IEND", &[]),
        ]
        .concat()
    }

    #[test]
    fn image_too_large_to_decode_is_stored_with_no_thumbnail() {
        let capture = classify(offer(&[("image/png", &png_header(40_000, 40_000))])).unwrap();
        assert_eq!(
            capture.content,
            ClipContent::Image(PixelSize {
                width: 40_000,
                height: 40_000
            })
        );
        assert_eq!(capture.thumbnail_png, None);
        assert_eq!(capture.payload.mimes().collect::<Vec<_>>(), ["image/png"]);
    }

    #[test]
    fn undecodable_image_falls_through_to_later_rules() {
        assert_eq!(
            kind(&[("image/png", b"garbage"), (UTF8_TEXT, b"caption")]),
            Ok(ClipKind::Text)
        );
        assert_eq!(kind(&[("image/png", b"garbage")]), Ok(ClipKind::Data));
    }

    #[test]
    fn pdf_media_and_data_kinds() {
        assert_eq!(
            kind(&[(PDF, b"%PDF-1.7"), (UTF8_TEXT, b"text")]),
            Ok(ClipKind::Pdf)
        );
        assert_eq!(kind(&[("video/mp4", b"....")]), Ok(ClipKind::Media));
        assert_eq!(kind(&[("audio/ogg", b"....")]), Ok(ClipKind::Media));
        assert_eq!(
            kind(&[("text/tab-separated-values", b"a\tb")]),
            Ok(ClipKind::Data)
        );
    }

    #[test]
    fn gtk_color_target_is_color() {
        let data: Vec<u8> = [65_535u16, 32_768, 0, 65_535]
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let capture = classify(offer(&[(COLOR, &data)])).unwrap();
        let ClipContent::Color(color) = capture.content else {
            panic!("not a color")
        };
        assert_eq!(color.hex(), "#FF8000");
    }

    #[test]
    fn rich_text_uses_plain_text_else_decoded_rtf() {
        let rtf: &[u8] =
            br"{\rtf1\ansi{\fonttbl\f0 Helvetica;}\f0 Hello \b bold\b0\par caf\'e9 \u8364?}";
        let with_text = classify(offer(&[("text/rtf", rtf), (UTF8_TEXT, b"Hello plain")])).unwrap();
        assert_eq!(
            with_text.content,
            ClipContent::RichText(TextSnippet::new("Hello plain"))
        );
        let only_rtf = classify(offer(&[("text/rtf", rtf), (UTF8_TEXT, b"  ")])).unwrap();
        assert_eq!(
            only_rtf.content,
            ClipContent::RichText(TextSnippet::new("Hello bold\ncafé €"))
        );
        assert_eq!(
            kind(&[("application/rtf", br"{\rtf1 {\fonttbl x;}}")]),
            Err(Skip::Blank)
        );
    }

    #[test]
    fn html_only_is_rich_text_with_tags_stripped() {
        let capture = classify(offer(&[(HTML, b"<p>Hello</p>\n<b>world</b>")])).unwrap();
        assert_eq!(
            capture.content,
            ClipContent::RichText(TextSnippet::new("Hello world"))
        );
        assert_eq!(kind(&[(HTML, b"<br><img src=x>")]), Err(Skip::Blank));
    }

    #[test]
    fn blank_text_is_skipped() {
        assert_eq!(kind(&[(UTF8_TEXT, b" \n\t ")]), Err(Skip::Blank));
    }

    #[test]
    fn concealed_selection_is_skipped() {
        assert_eq!(
            kind(&[(UTF8_TEXT, b"hunter2"), (KDE_PASSWORD_HINT, b"secret")]),
            Err(Skip::Concealed)
        );
        assert_eq!(
            kind(&[
                (UTF8_TEXT, b"hunter2"),
                ("org.nspasteboard.ConcealedType", b"")
            ]),
            Err(Skip::Concealed)
        );
        assert_eq!(
            kind(&[(UTF8_TEXT, b"hunter2"), (KDE_PASSWORD_HINT, b"secret\n")]),
            Err(Skip::Concealed)
        );
        let note = classify(offer(&[(UTF8_TEXT, b"note"), (KDE_PASSWORD_HINT, b"other")])).unwrap();
        assert_eq!(note.content.kind(), ClipKind::Text);
        assert_eq!(note.payload, offer(&[(UTF8_TEXT, b"note")]));
    }

    #[test]
    fn selection_over_50_mib_is_skipped() {
        let big = vec![b'a'; MAX_CAPTURE_BYTES as usize + 1];
        assert_eq!(kind(&[(UTF8_TEXT, &big)]), Err(Skip::TooLarge));
    }

    #[test]
    fn unknown_or_empty_targets_store_nothing() {
        assert_eq!(
            kind(&[("TARGETS", b"x"), (UTF8_TEXT, b"")]),
            Err(Skip::NothingStored)
        );
    }

    #[test]
    fn hash_ignores_target_order_text_aliases_and_volatile_data() {
        let a = classify(offer(&[
            (HTML, b"<b>x</b>"),
            ("UTF8_STRING", b"x"),
            ("chromium/x-web-custom-data", b"1"),
        ]))
        .unwrap();
        let b = classify(offer(&[
            (UTF8_TEXT, b"x"),
            ("chromium/x-web-custom-data", b"2"),
            (HTML, b"<b>x</b>"),
        ]))
        .unwrap();
        assert_eq!(a.content_hash, b.content_hash);
        let c = classify(offer(&[(UTF8_TEXT, b"y"), (HTML, b"<b>x</b>")])).unwrap();
        assert_ne!(a.content_hash, c.content_hash);
    }

    #[test]
    fn hash_is_stable_across_releases() {
        let capture = classify(offer(&[("text/plain", b"hello")])).unwrap();
        assert_eq!(
            capture.content_hash.0,
            "2a8602a46827ffc664cb3d16c1533f292eb4b20bc7d18630abc747ea9e28f1ca"
        );
    }
}
