namespace IsomFolio.Extensions.Sdk;

public static class SdkArgs
{
    public static bool IsSetupMode(string[] args) => args.Length > 0 && args[0] == "setup";

    /// Persistent data directory passed by the host via `--data-dir <path>`.
    /// Falls back to `AppContext.BaseDirectory` when the argument is absent (e.g. in tests).
    public static string DataDir(string[] args)
    {
        for (var i = 0; i < args.Length - 1; i++)
        {
            if (args[i] == "--data-dir")
                return args[i + 1];
        }
        return AppContext.BaseDirectory;
    }

    /// Catalog DB path passed by the host via `--catalog-db <path>`.
    /// Null when the argument is absent (non-catalog-aware extensions and tests).
    public static string? CatalogDbPath(string[] args)
    {
        for (var i = 0; i < args.Length - 1; i++)
        {
            if (args[i] == "--catalog-db")
                return args[i + 1];
        }
        return null;
    }
}
