using System.Text.Json;

namespace IsomFolio.Addons.Faces;

public record DbscanConfig(float Eps = 0.4f, int MinPts = 2);

public interface IDbscanConfigFactory
{
    DbscanConfig Create();
}

internal class DbscanConfigFactory(IMessageOutbox outbox) : IDbscanConfigFactory
{
    public DbscanConfig Create()
    {
        var path = Environment.GetEnvironmentVariable("ISOMFOLIO_ADDON_CONFIG") ?? "";
        if (string.IsNullOrEmpty(path) || !File.Exists(path))
            return new DbscanConfig();

        try
        {
            var json = File.ReadAllText(path);
            return JsonSerializer.Deserialize(json, AppJsonContext.Default.DbscanConfig) ?? new DbscanConfig();
        }
        catch (Exception ex)
        {
            outbox.SendLog(LogLevel.Warning, $"config parse error: {ex.Message}, using defaults");
            return new DbscanConfig();
        }
    }
}