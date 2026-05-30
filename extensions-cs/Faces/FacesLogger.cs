using System.Text.Json;
using System.Text.Json.Serialization;
using IsomFolio.Extensions.Sdk;

namespace IsomFolio.Extensions.Faces;

internal sealed class FacesLogger(string component)
{
    public async ValueTask LogAsync(LogLevel level, string message)
    {
        await Console.Error.WriteLineAsync(Serialize(level, message));
        await Console.Error.FlushAsync();
    }

    public void Log(LogLevel level, string message)
    {
        Console.Error.WriteLine(Serialize(level, message));
        Console.Error.Flush();
    }

    private string Serialize(LogLevel level, string message) =>
        JsonSerializer.Serialize(
            new LogEntry(LevelTag(level), component, message),
            FacesLogContext.Default.LogEntry);

    private static string LevelTag(LogLevel level) => level switch
    {
        LogLevel.Error => "error",
        LogLevel.Warning => "warning",
        _ => "info"
    };
}

internal record LogEntry(string Level, string Component, string Message);

[JsonSerializable(typeof(LogEntry))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
internal partial class FacesLogContext : JsonSerializerContext;
