using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Sdk;

[JsonConverter(typeof(JsonStringEnumConverter<AddonCapability>))]
public enum AddonCapability
{
    [JsonStringEnumMemberName("cluster_faces")]
    ClusterFaces,
    [JsonStringEnumMemberName("classify")]
    Classify,
}
