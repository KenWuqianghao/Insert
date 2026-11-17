using System.Globalization;

namespace Insert.Core;

public static class Format
{
    public static string Counted(long count, string noun) =>
        $"{count.ToString("N0", CultureInfo.InvariantCulture)} {noun}{(count == 1 ? "" : "s")}";

    /// <summary>Decimal units, like the file sizes in File Explorer details of macOS.</summary>
    public static string Bytes(long count)
    {
        if (count < 1_000) return Counted(count, "byte");
        string[] units = ["KB", "MB", "GB", "TB"];
        double value = count;
        var unit = -1;
        while (value >= 1_000 && unit < units.Length - 1)
        {
            value /= 1_000;
            unit++;
        }
        var digits = value < 10 ? "0.#" : "0";
        return $"{value.ToString(digits, CultureInfo.InvariantCulture)} {units[unit]}";
    }

    /// <summary>The last part of a Windows or Unix path.</summary>
    public static string FileName(string path)
    {
        var trimmed = path.TrimEnd('\\', '/');
        var index = trimmed.LastIndexOfAny(['\\', '/']);
        return index < 0 ? trimmed : trimmed[(index + 1)..];
    }

    public static string? DirectoryName(string path)
    {
        var trimmed = path.TrimEnd('\\', '/');
        var index = trimmed.LastIndexOfAny(['\\', '/']);
        return index <= 0 ? null : trimmed[..index];
    }

    public static string RelativeTime(DateTimeOffset date, DateTimeOffset now)
    {
        var seconds = Math.Max(0, (long)(now - date).TotalSeconds);
        if (seconds < 60) return "Just now";

        var minutes = seconds / 60;
        if (minutes < 60) return $"{minutes} min ago";

        var hours = minutes / 60;
        if (hours < 24) return $"{hours} hr ago";

        var days = hours / 24;
        if (days < 7) return $"{days} {(days == 1 ? "day" : "days")} ago";

        var weeks = days / 7;
        if (weeks < 5) return $"{weeks} {(weeks == 1 ? "week" : "weeks")} ago";

        var months = days / 30;
        if (months < 12) return $"{months} mo ago";

        return $"{days / 365} yr ago";
    }
}
