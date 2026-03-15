namespace Anemoi.Core.Models;

public enum BackendType
{
    Ollama = 1,
    LlamaCpp = 2,
    FoundryLocal = 3
}

public enum ExecutionTarget
{
    Local = 1,
    Cloud = 2,
    Hybrid = 3
}

public enum ExecutionMode
{
    Primary = 1,
    Fallback = 2
}

public sealed record RouterChatRequest(
    string? RequestedModel,
    IReadOnlyCollection<RouterMessage> Messages,
    bool Stream,
    double? Temperature,
    double? TopP,
    int? MaxTokens,
    IReadOnlyDictionary<string, string>? Metadata = null);

public sealed record RouterMessage(string Role, string Content, string? Name = null);

public sealed record RouterChatResponse(
    string ResponseId,
    string ObjectType,
    long CreatedAtEpochSeconds,
    string Model,
    IReadOnlyCollection<RouterChoice> Choices,
    RouterUsage? Usage);

public sealed record RouterChoice(int Index, RouterMessage Message, string? FinishReason);

public sealed record RouterUsage(int PromptTokens, int CompletionTokens, int TotalTokens);

public sealed record RouteDecision(
    string SelectedAlias,
    string SelectedProfile,
    string SelectedBackend,
    string UpstreamModel,
    double? Temperature,
    double? TopP,
    int? MaxTokens,
    string? RoutingReason,
    string? FallbackAlias);

public sealed record BackendDescriptor(
    string Id,
    BackendType Type,
    Uri BaseUrl,
    TimeSpan Timeout,
    bool Enabled,
    bool AllowInsecureTls,
    IReadOnlyDictionary<string, string> Metadata);

public sealed record BackendHealthResult(
    string BackendId,
    BackendType BackendType,
    bool IsHealthy,
    string Status,
    DateTimeOffset CheckedAtUtc,
    string? Details = null);

public sealed record ProfileDefinition(
    string ProfileId,
    string BackendId,
    string UpstreamModel,
    double? Temperature,
    double? TopP,
    int? MaxTokens,
    int CapabilityScore,
    ExecutionTarget ExecutionTarget);

public sealed record AliasDefinition(
    string Alias,
    string ProfileId,
    string? FallbackAlias,
    bool VisibleToUi);

public sealed class RouterRequestContext
{
    public string RequestId { get; init; } = Guid.NewGuid().ToString("N");

    public DateTimeOffset StartedAtUtc { get; init; } = DateTimeOffset.UtcNow;

    public string? SelectedAlias { get; set; }

    public string? SelectedProfile { get; set; }

    public string? SelectedBackend { get; set; }

    public IDictionary<string, string> Metadata { get; } = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
}

public sealed record RouterStreamEvent(
    string ResponseId,
    long CreatedAtEpochSeconds,
    string Model,
    int ChoiceIndex,
    string? DeltaRole,
    string? DeltaContent,
    string? FinishReason,
    bool IsTerminal = false);

public sealed record ChatExecutionResult(
    RouteDecision RouteDecision,
    RouterChatResponse Response,
    ExecutionMode ExecutionMode,
    bool FallbackUsed);

public sealed record StreamingChatExecutionResult(
    RouteDecision RouteDecision,
    IAsyncEnumerable<RouterStreamEvent> Events,
    ExecutionMode ExecutionMode,
    bool FallbackUsed);
