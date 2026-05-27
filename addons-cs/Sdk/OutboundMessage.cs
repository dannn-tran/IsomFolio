using System.Text.Json.Serialization;

namespace IsomFolio.Addons.Sdk;

public abstract record OutboundMessage;

[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(LogMessage), "log")]
[JsonDerivedType(typeof(ProgressMessage), "progress")]
[JsonDerivedType(typeof(ReadyMessage), "ready")]
public abstract record OutboundEvent : OutboundMessage;

public record LogMessage(LogLevel Level, string Message) : OutboundEvent;
public record ProgressMessage(ulong Id, int Percent) : OutboundEvent;
public record ReadyMessage : OutboundEvent;

public record OkResponse<TResult>(ulong Id, TResult Result) : OutboundMessage
{
    public string Type { get; init; } = "ok";
}

public record ErrorResponse(ulong Id, string Error) : OutboundMessage
{
    public string Type { get; init; } = "error";
}
