namespace IsomFolio.Addons.Sdk;

public static class SdkArgs
{
    public static string ModelsDir() => Path.Combine(AppContext.BaseDirectory, "models");
    public static bool IsSetupMode(string[] args) => args.Length > 0 && args[0] == "setup";
}
