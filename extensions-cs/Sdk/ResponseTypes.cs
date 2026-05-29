namespace IsomFolio.Extensions.Sdk;

public record HandshakeResult(int ProtocolVersion, string ExtensionVersion, ExtensionCapability[] Capabilities);
public record PingResult;

public record ClassifyResult(string FileId, List<TagScore> Tags);
public record TagScore(string Tag, float Confidence);

public record ClusterResult(List<ClusterEntry> Clusters, List<FaceMember> Noise);
public record ClusterEntry(string Id, List<FaceMember> Members);
public record FaceMember(string FileId, BboxData Bbox);
public record BboxData(float X, float Y, float W, float H);
