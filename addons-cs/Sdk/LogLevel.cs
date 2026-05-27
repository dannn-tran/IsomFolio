using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Sdk;

[JsonConverter(typeof(JsonStringEnumConverter<LogLevel>))]
public enum LogLevel
{
    [JsonStringEnumMemberName("info")] Info,
    [JsonStringEnumMemberName("warning")] Warning,
    [JsonStringEnumMemberName("error")] Error
}
