using Anemoi.Core.Models;

namespace Anemoi.Core.Interfaces;

public interface IChatBackend
{
    BackendDescriptor Descriptor { get; }

    Task<RouterChatResponse> CompleteChatAsync(
        RouterChatRequest request,
        RouteDecision decision,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken);

    IAsyncEnumerable<RouterStreamEvent> StreamChatAsync(
        RouterChatRequest request,
        RouteDecision decision,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken);

    Task<BackendHealthResult> CheckHealthAsync(CancellationToken cancellationToken);
}

public interface IBackendRegistry
{
    IChatBackend GetBackend(string backendId);

    IReadOnlyCollection<IChatBackend> GetAllBackends();
}

public interface IRouteSelector
{
    RouteDecision SelectRoute(RouterChatRequest request);

    RouteDecision? ResolveFallbackRoute(RouteDecision currentDecision);
}

public interface IProfileResolver
{
    AliasDefinition ResolveAlias(string alias);

    bool TryResolveAlias(string alias, out AliasDefinition? aliasDefinition);

    ProfileDefinition ResolveProfile(string profileId);

    IReadOnlyCollection<AliasDefinition> GetVisibleAliases();
}

public interface IChatCompletionService
{
    Task<ChatExecutionResult> CompleteAsync(
        RouterChatRequest request,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken);

    StreamingChatExecutionResult Stream(
        RouterChatRequest request,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken);
}

public interface IBackendHealthService
{
    Task<IReadOnlyCollection<BackendHealthResult>> GetBackendHealthAsync(CancellationToken cancellationToken);
}
