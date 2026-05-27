using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Faces;

[JsonSerializable(typeof(MessageInbound))]
[JsonSerializable(typeof(MessageOutbound))]
[JsonSerializable(typeof(DbscanConfig))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
public partial class AppJsonContext : JsonSerializerContext;