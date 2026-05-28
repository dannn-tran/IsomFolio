namespace IsomFolio.Addons.Sdk;

public static class SdkArgs
{
    public static string? DataDir(string[] args) => Get(args, "--data-dir");
    public static bool IsInstallMode(string[] args) => args.Length > 0 && args[0] == "install";

    private static string? Get(string[] args, string name)
    {
        for (var i = 0; i < args.Length - 1; i++)
            if (args[i] == name) return args[i + 1];
        return null;
    }
}
