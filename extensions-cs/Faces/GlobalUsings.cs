// The Web SDK's implicit usings bring in Microsoft.Extensions.Logging.LogLevel,
// which collides with the SDK's diagnostic LogLevel used by FacesLogger. Pin the
// name to ours everywhere.
global using LogLevel = IsomFolio.Extensions.Sdk.LogLevel;
global using SessionOptions = Microsoft.ML.OnnxRuntime.SessionOptions;
