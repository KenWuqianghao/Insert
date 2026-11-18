using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Media;
using Insert.Core;

namespace Insert.Views;

/// <summary>The context menus of the tray. Each one is built when it opens, so it shows the current pinboards.</summary>
public static class Menus
{
    public static ContextMenu ForClip(PanelModel model, Clip clip)
    {
        var library = model.Library;
        var targets = model.Targets(clip);

        var pinTo = Item("Pin to");
        var pinItems = library.Pinboards.Select(pinboard => (object)Item(pinboard.Name, () => library.SetPinboard(pinboard.Id, targets), Dot(pinboard.Color))).ToList();
        if (pinItems.Count > 0) pinItems.Add(new Separator());
        pinItems.Add(Item("New Pinboard…", () => model.BeginNaming(new NameTarget.NewPinboard(targets))));
        pinTo.ItemsSource = pinItems;

        var items = new List<object>
        {
            Item("Paste", () => model.ActivateClip(clip, PasteAction.Paste)),
            Item("Paste as Plain Text", () => model.ActivateClip(clip, PasteAction.PastePlainText)),
            Item("Copy", () => model.ActivateClip(clip, PasteAction.Copy)),
            new Separator(),
            pinTo,
        };
        if (clip.PinboardId is not null) items.Add(Item("Unpin", () => library.SetPinboard(null, targets)));
        items.Add(Item("Rename…", () => model.BeginNaming(new NameTarget.ClipName(clip.Id))));
        items.Add(Item("Preview", () => model.Preview(clip)));
        items.Add(new Separator());
        items.Add(Item("Delete", () => model.Delete(targets)));
        return new ContextMenu { ItemsSource = items };
    }

    public static ContextMenu ForPinboard(PanelModel model, Pinboard pinboard)
    {
        var library = model.Library;
        var color = Item("Color");
        color.ItemsSource = Enum.GetValues<PinboardColor>().Select(value =>
        {
            var item = Item(value.ToString(), () => library.SetPinboardColor(pinboard.Id, value), Dot(value));
            item.ToggleType = MenuItemToggleType.Radio;
            item.IsChecked = value == pinboard.Color;
            return item;
        }).ToList();

        return new ContextMenu
        {
            ItemsSource = new List<object>
            {
                Item("Rename…", () => model.BeginNaming(new NameTarget.PinboardName(pinboard.Id))),
                color,
                new Separator(),
                Item("Delete Pinboard", () => library.DeletePinboard(pinboard.Id)),
            },
        };
    }

    public static ContextMenu ForKindFilter(PanelModel model)
    {
        var items = new List<object> { Radio("All Types", model.KindFilter is null, () => model.KindFilter = null, null), new Separator() };
        items.AddRange(ClipKinds.All.Select(kind => Radio(kind.Name(), model.KindFilter == kind, () => model.KindFilter = kind, Palette.KindIcon(kind))));
        return new ContextMenu { ItemsSource = items };
    }

    public static ContextMenu More(bool capturePaused, Action settings, Action togglePause, Action clearHistory, Action quit) => new()
    {
        ItemsSource = new List<object>
        {
            Item("Settings…", settings),
            Item(capturePaused ? "Resume Capture" : "Pause Capture", togglePause),
            Item("Clear History…", clearHistory),
            new Separator(),
            Item("Quit Insert", quit),
        },
    };

    private static MenuItem Item(string header, Action? action = null, object? icon = null)
    {
        var item = new MenuItem { Header = header, Icon = icon };
        if (action is not null) item.Click += (_, _) => action();
        return item;
    }

    private static MenuItem Radio(string header, bool isChecked, Action action, Geometry? glyph)
    {
        var item = Item(header, action, glyph is null ? null : new PathIcon { Data = glyph, Width = 14, Height = 14 });
        item.ToggleType = MenuItemToggleType.Radio;
        item.IsChecked = isChecked;
        return item;
    }

    private static Ellipse Dot(PinboardColor color) => new() { Width = 8, Height = 8, Fill = Palette.PinboardBrush(color) };
}
