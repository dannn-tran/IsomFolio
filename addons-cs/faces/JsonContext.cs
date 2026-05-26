using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Faces;

[JsonSerializable(typeof(HelloMessage))]
[JsonSerializable(typeof(ResponseMessage))]
[JsonSerializable(typeof(ErrorMessage))]
[JsonSerializable(typeof(LogMessage))]
[JsonSerializable(typeof(ProgressMessage))]
[JsonSerializable(typeof(ClusterResult))]
[JsonSerializable(typeof(ClusterEntry))]
[JsonSerializable(typeof(FaceMember))]
[JsonSerializable(typeof(BboxData))]
[JsonSerializable(typeof(AddonRequest))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
public partial class AppJsonContext : JsonSerializerContext;

public record HelloMessage(string Type, int ProtocolVersion, int AddonApiVersion, string[] Capabilities);
public record ResponseMessage(ulong Id, ClusterResult Result);
public record ErrorMessage(ulong Id, string Error);
public record LogMessage(string Type, string Level, string Message);
public record ProgressMessage(string Type, ulong Id, int Percent);
public record ClusterResult(List<ClusterEntry> Clusters, List<FaceMember> Noise);
public record ClusterEntry(string Id, List<FaceMember> Members);
public record FaceMember(string FileId, BboxData Bbox);
public record BboxData(float X, float Y, float W, float H);
public record AddonRequest(ulong Id, string Method, System.Text.Json.JsonElement Params);
