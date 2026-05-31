namespace IsomFolio.Extensions.Sdk;

public record HandshakeResult(int ProtocolVersion, string ExtensionVersion, ExtensionCapability[] Capabilities);
public record PingResult;

public record ClassifyResult(string FileId, List<TagScore> Tags);
public record TagScore(string Tag, float Confidence);
