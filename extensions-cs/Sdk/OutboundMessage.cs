using System.Text.Json.Serialization;

namespace IsomFolio.Extensions.Sdk;

public abstract record OutboundMessage;

[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(ProgressMessage), "progress")]
[JsonDerivedType(typeof(ReadyMessage), "ready")]
[JsonDerivedType(typeof(FatalMessage), "fatal")]
public abstract record OutboundEvent : OutboundMessage;

public record ProgressMessage(ulong Id, int Percent) : OutboundEvent;
public record ReadyMessage : OutboundEvent;
public record FatalMessage(bool Repairable, string Message) : OutboundEvent;

public record OkResponse<TResult>(ulong Id, TResult Result) : OutboundMessage
{
    public string Type { get; init; } = "ok";
}

public record ErrorResponse(ulong Id, string Error) : OutboundMessage
{
    public string Type { get; init; } = "error";
}
