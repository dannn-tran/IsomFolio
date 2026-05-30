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
}
