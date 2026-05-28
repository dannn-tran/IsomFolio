using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Sdk;

[JsonConverter(typeof(JsonStringEnumConverter<ExtensionCapability>))]
public enum ExtensionCapability
{
    [JsonStringEnumMemberName("cluster_faces")]
    ClusterFaces,
    [JsonStringEnumMemberName("classify")]
    Classify,
}
