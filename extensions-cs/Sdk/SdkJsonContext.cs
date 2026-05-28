using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Sdk;

[JsonSerializable(typeof(ExtensionCapability))]
[JsonSerializable(typeof(InboundMessage))]
[JsonSerializable(typeof(OutboundEvent))]
[JsonSerializable(typeof(OkResponse<HandshakeResult>))]
[JsonSerializable(typeof(OkResponse<PingResult>))]
[JsonSerializable(typeof(OkResponse<ClassifyResult>))]
[JsonSerializable(typeof(OkResponse<ClusterResult>))]
[JsonSerializable(typeof(ErrorResponse))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower, AllowOutOfOrderMetadataProperties = true)]
public partial class SdkJsonContext : JsonSerializerContext;
