using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Faces;


[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(HelloMessage), typeDiscriminator: "hello")]
[JsonDerivedType(typeof(LogMessage), typeDiscriminator: "log")]
[JsonDerivedType(typeof(ProgressMessage), typeDiscriminator: "progress")]
[JsonDerivedType(typeof(ErrorMessage), typeDiscriminator: "error")]
[JsonDerivedType(typeof(ResponseMessage), typeDiscriminator: "response")]
public abstract record MessageOutbound;

public record HelloMessage(int ProtocolVersion, int AddonApiVersion, string[] Capabilities) : MessageOutbound;
public record LogMessage(
    [property: JsonConverter(typeof(JsonStringEnumConverter<LogLevel>))]
    LogLevel Level,
    string Message) : MessageOutbound;
public record ProgressMessage(ulong Id, int Percent);
public record ErrorMessage(ulong Id, string Error) : MessageOutbound;
public record ResponseMessage(ulong Id, ClusterResult Result) : MessageOutbound;


public enum LogLevel
{
    Info,
    Warning,
    Error
}

public record ClusterResult(List<ClusterEntry> Clusters, List<FaceMember> Noise);
public record ClusterEntry(string Id, List<FaceMember> Members);
public record FaceMember(string FileId, BboxData Bbox);
public record BboxData(float X, float Y, float W, float H);