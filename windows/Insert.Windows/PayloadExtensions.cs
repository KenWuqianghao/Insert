using System;
using System.Collections.Specialized;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Windows.Forms;

namespace Insert.Windows;

static class PayloadExtensions
{
    public static DataObject ToDataObject(this ClipboardPayload payload)
    {
        var data = new DataObject();

        if (!string.IsNullOrEmpty(payload.Text))
        {
            data.SetText(payload.Text);
        }

        if (!string.IsNullOrEmpty(payload.Html))
        {
            data.SetData(DataFormats.Html, payload.Html);
        }

        if (!string.IsNullOrEmpty(payload.Rtf))
        {
            data.SetData(DataFormats.Rtf, payload.Rtf);
        }

        if (!string.IsNullOrEmpty(payload.Url))
        {
            data.SetData(DataFormats.UnicodeText, payload.Url);
        }

        if (payload.Files is { Count: > 0 })
        {
            var collection = new StringCollection();
            collection.AddRange(payload.Files.ToArray());
            data.SetFileDropList(collection);
        }

        if (!string.IsNullOrEmpty(payload.ImageBase64))
        {
            var bytes = Convert.FromBase64String(payload.ImageBase64);
            using var ms = new MemoryStream(bytes);
            using var image = Image.FromStream(ms);
            data.SetImage((Image)image.Clone());
        }

        if (!string.IsNullOrEmpty(payload.DataBase64) && !string.IsNullOrEmpty(payload.DataFormat))
        {
            data.SetData(payload.DataFormat, Convert.FromBase64String(payload.DataBase64));
        }

        if (payload.ExtraRepresentations is { Count: > 0 })
        {
            foreach (var representation in payload.ExtraRepresentations)
            {
                if (!string.IsNullOrEmpty(representation.Text))
                {
                    data.SetData(representation.Format, representation.Text);
                }
                else if (!string.IsNullOrEmpty(representation.Base64))
                {
                    data.SetData(representation.Format, Convert.FromBase64String(representation.Base64));
                }
            }
        }

        return data;
    }
}
