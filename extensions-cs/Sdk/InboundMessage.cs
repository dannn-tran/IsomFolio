using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Sdk;

[JsonPolymorphic(TypeDiscriminatorPropertyName = "method")]
[JsonDerivedType(typeof(HandshakeRequest), "handshake")]
[JsonDerivedType(typeof(PingRequest), "ping")]
[JsonDerivedType(typeof(ClassifyRequest), "classify")]
public abstract record InboundMessage(ulong Id);

public record HandshakeRequest(ulong Id) : InboundMessage(Id);
public record PingRequest(ulong Id) : InboundMessage(Id);

public record ClassifyRequest(ulong Id, ClassifyRequestParams Params) : InboundMessage(Id);
public record ClassifyRequestParams(string FileId, string ThumbnailPath);
