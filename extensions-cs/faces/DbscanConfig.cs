using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Faces;

public record DbscanConfig(float Eps = 0.4f, int MinPts = 2);

[JsonSerializable(typeof(DbscanConfig))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
public partial class AppJsonContext : JsonSerializerContext;