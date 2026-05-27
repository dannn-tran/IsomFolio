using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Faces;


[JsonPolymorphic(TypeDiscriminatorPropertyName = "method")]
[JsonDerivedType(typeof(ClusterFacesRequest), typeDiscriminator: "cluster_faces")]
public abstract record MessageInbound(ulong Id);

public record ClusterFacesRequest(ulong Id, ClusterFacesRequestParams Params) : MessageInbound(Id);

public record ClusterFacesRequestParams(ImageInfo[] Files, bool ForceFull);
public record ImageInfo(string FileId, string ImagePath, long FileMtime);
