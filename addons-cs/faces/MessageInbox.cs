using System.Diagnostics.CodeAnalysis;
using System.Text.Json;

namespace IsomFolio.Addons.Faces;

public static class MessageInbox
{
    public static bool TryGetNext([MaybeNullWhen(false)] out MessageInbound msg)
    {
        while (true)
        {
            if (Console.ReadLine() is not { } line)
            {
                msg = null;
                return false;
            }

            line = line.Trim();
            if (string.IsNullOrEmpty(line)) continue;

            var req = JsonSerializer.Deserialize(line, AppJsonContext.Default.MessageInbound);
            if (req != null)
            {
                msg = req;
                return true;
            }

            Console.Error.WriteLine($"[faces] parse error: {line}");
        }
    }
}