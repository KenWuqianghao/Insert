use time::OffsetDateTime;
use unicode_segmentation::UnicodeSegmentation;

/// Coarse relative time for card headers ("Just now", "5 min ago", "3 days ago").
pub fn relative_time(created_at: OffsetDateTime, now: OffsetDateTime) -> String {
    let seconds = (now - created_at).whole_seconds().max(0);
    if seconds < 60 {
        return "Just now".to_owned();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes} min ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours} hr ago");
    }
    let days = hours / 24;
    if days < 7 {
        return format!("{days} {} ago", if days == 1 { "day" } else { "days" });
    }
    let weeks = days / 7;
    if weeks < 5 {
        return format!("{weeks} {} ago", if weeks == 1 { "week" } else { "weeks" });
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months} mo ago");
    }
    format!("{} yr ago", (days / 365).max(1))
}

/// An integer with comma grouping, for example "1,234".
pub fn format_count(count: usize) -> String {
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// A file size in decimal units, for example "345 KB" or "1.2 MB".
pub fn format_bytes(bytes: u64) -> String {
    match bytes {
        0 => return "Zero KB".to_owned(),
        1 => return "1 byte".to_owned(),
        2..=999 => return format!("{bytes} bytes"),
        _ => {}
    }
    const UNITS: [(&str, usize); 4] = [("KB", 0), ("MB", 1), ("GB", 2), ("TB", 2)];
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 999.5 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let (name, decimals) = UNITS[unit];
    let number = format!("{value:.decimals$}");
    let number = if decimals > 0 {
        number.trim_end_matches('0').trim_end_matches('.')
    } else {
        &number
    };
    format!("{number} {name}")
}

pub fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

pub fn grapheme_prefix(text: &str, count: usize) -> &str {
    match text.grapheme_indices(true).nth(count) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn ago(seconds: i64) -> String {
        let now = OffsetDateTime::UNIX_EPOCH + Duration::days(1000);
        relative_time(now - Duration::seconds(seconds), now)
    }

    #[test]
    fn relative_time_steps() {
        const DAY: i64 = 86_400;
        assert_eq!(ago(-30), "Just now");
        assert_eq!(ago(59), "Just now");
        assert_eq!(ago(60), "1 min ago");
        assert_eq!(ago(5 * 60 + 59), "5 min ago");
        assert_eq!(ago(3 * 3_600), "3 hr ago");
        assert_eq!(ago(DAY), "1 day ago");
        assert_eq!(ago(6 * DAY), "6 days ago");
        assert_eq!(ago(7 * DAY), "1 week ago");
        assert_eq!(ago(34 * DAY), "4 weeks ago");
        assert_eq!(ago(35 * DAY), "1 mo ago");
        assert_eq!(ago(359 * DAY), "11 mo ago");
        assert_eq!(ago(800 * DAY), "2 yr ago");
    }

    #[test]
    fn days_360_to_364_say_one_year() {
        for days in 360..=364 {
            assert_eq!(ago(days * 86_400), "1 yr ago");
        }
    }

    #[test]
    fn counts_and_bytes() {
        assert_eq!(format_count(7), "7");
        assert_eq!(format_count(1_234), "1,234");
        assert_eq!(format_count(1_234_567), "1,234,567");
        assert_eq!(format_bytes(0), "Zero KB");
        assert_eq!(format_bytes(512), "512 bytes");
        assert_eq!(format_bytes(345_000), "345 KB");
        assert_eq!(format_bytes(1_200_000), "1.2 MB");
        assert_eq!(format_bytes(2_000_000), "2 MB");
        assert_eq!(format_bytes(3_450_000_000), "3.45 GB");
    }

    #[test]
    fn grapheme_prefix_cuts_on_clusters() {
        assert_eq!(grapheme_prefix("e\u{301}xyz", 2), "e\u{301}x");
        assert_eq!(grapheme_prefix("ab", 5), "ab");
    }
}
