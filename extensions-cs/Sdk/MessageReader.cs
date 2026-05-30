using System.Runtime.CompilerServices;
using System.Text.Json;

namespace IsomFolio.Extensions.Sdk;

public static class MessageReader
{
    public static async IAsyncEnumerable<InboundMessage> ReadAllAsync(
        TextReader input,
        [EnumeratorCancellation] CancellationToken ct = default)
    {
        while (true)
        {
            string? line;
            try
            {
                line = await input.ReadLineAsync(ct);
            }
            catch (OperationCanceledException)
            {
                yield break;
            }

            if (line == null) yield break;
            line = line.Trim();
            if (string.IsNullOrEmpty(line)) continue;

            InboundMessage? msg = null;
            try
            {
                msg = JsonSerializer.Deserialize(line, SdkJsonContext.Default.InboundMessage);
            }
            catch
            {
                Console.Error.WriteLine($"[warning] protocol: parse error: {line}");
                Console.Error.Flush();
            }

            if (msg != null) yield return msg;
        }
    }
}
