using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Faces.Dto;


[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(HelloMessage), typeDiscriminator: "hello")]
[JsonDerivedType(typeof(ErrorMessage), typeDiscriminator: "error")]
[JsonDerivedType(typeof(ResponseMessage), typeDiscriminator: "response")]
[JsonDerivedType(typeof(LogMessage), typeDiscriminator: "log")]
public abstract record BaseMessage;