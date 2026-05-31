using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Sdk;

[JsonConverter(typeof(JsonStringEnumConverter<ExtensionCapability>))]
public enum ExtensionCapability
{
    [JsonStringEnumMemberName("classify")]
    Classify,
}
