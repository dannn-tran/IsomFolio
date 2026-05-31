using System.Text.Json;
using IsomFolio.Extensions.Faces;
using IsomFolio.Extensions.Sdk;

// Face inference engine: a localhost HTTP server exposing GET /health and
// POST /embed. The host spawns this, waits for /health, then streams batches
// of files to /embed. See dev-docs/face-inference-engine.md.

var dataDir = SdkArgs.DataDir(args);
var port = ParsePort(args);
// Bind loopback by default (managed local engine). In a container we need
// 0.0.0.0 so the published port is reachable from the host.
var bindAnyIp = ParseHost(args) == "0.0.0.0";
// Precedence: --variant arg > config.json (host config UI) > env > default.
var variant = ModelVariant.Current(ParseVariant(args) ?? ReadConfiguredVariant());

var crashLog = new FacesLogger("fatal");
AppDomain.CurrentDomain.UnhandledException += (_, e) =>
    crashLog.Log(LogLevel.Error, $"unhandled: {e.ExceptionObject}");

var engine = new Engine();
// Load models in the background so /health can report 503 (loading) / 500
// (failed) while the socket is already accepting connections.
_ = engine.LoadAsync(dataDir, variant);

var builder = WebApplication.CreateSlimBuilder(args);
builder.Logging.ClearProviders();
builder.WebHost.ConfigureKestrel(o =>
{
    if (bindAnyIp) o.ListenAnyIP(port);
    else o.ListenLocalhost(port);
});
builder.Services.ConfigureHttpJsonOptions(o =>
    o.SerializerOptions.TypeInfoResolverChain.Insert(0, EngineJsonContext.Default));

var app = builder.Build();

app.MapGet("/health", () =>
{
    if (engine.FatalError is { } err) return Results.Text(err, "text/plain", null, 500);
    return engine.Ready ? Results.Ok() : Results.StatusCode(503);
});

app.MapPost("/embed", async (EmbedRequest req, CancellationToken ct) =>
{
    if (!engine.Ready) return Results.StatusCode(503);
    return Results.Ok(await engine.EmbedAsync(req, ct));
});

app.Run();

static int ParsePort(string[] args)
{
    for (var i = 0; i < args.Length - 1; i++)
        if (args[i] == "--port" && int.TryParse(args[i + 1], out var p))
            return p;
    return 45876;
}

static string ParseHost(string[] args)
{
    for (var i = 0; i < args.Length - 1; i++)
        if (args[i] == "--host")
            return args[i + 1];
    return "127.0.0.1";
}

static string? ParseVariant(string[] args)
{
    for (var i = 0; i < args.Length - 1; i++)
        if (args[i] == "--variant")
            return args[i + 1];
    return null;
}

static string? ReadConfiguredVariant()
{
    var path = Path.Combine(AppContext.BaseDirectory, "config.json");
    if (!File.Exists(path)) return null;
    try
    {
        var cfg = JsonSerializer.Deserialize(File.ReadAllText(path), EngineJsonContext.Default.EngineConfig);
        return cfg?.ModelVariant;
    }
    catch { return null; }
}
