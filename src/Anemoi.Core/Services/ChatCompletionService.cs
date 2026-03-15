using System.Diagnostics;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Anemoi.Core.Configuration;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Core.Services;

public sealed class ChatCompletionService : IChatCompletionService
{
    private readonly IBackendRegistry _backendRegistry;
    private readonly IRouteSelector _routeSelector;
    private readonly RouterOptions _routerOptions;
    private readonly ILogger<ChatCompletionService> _logger;

    public ChatCompletionService(
        IBackendRegistry backendRegistry,
        IRouteSelector routeSelector,
        IOptions<RouterOptions> routerOptions,
        ILogger<ChatCompletionService> logger)
    {
        _backendRegistry = backendRegistry;
        _routeSelector = routeSelector;
        _routerOptions = routerOptions.Value;
        _logger = logger;
    }

    public Task<ChatExecutionResult> CompleteAsync(
        RouterChatRequest request,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken)
    {
        var route = _routeSelector.SelectRoute(request);
        return ExecuteNonStreamingAsync(request, requestContext, route, ExecutionMode.Primary, false, cancellationToken);
    }

    public StreamingChatExecutionResult Stream(
        RouterChatRequest request,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken)
    {
        var route = _routeSelector.SelectRoute(request);
        return new StreamingChatExecutionResult(
            route,
            ExecuteStreamingAsync(request, requestContext, route, ExecutionMode.Primary, false, cancellationToken),
            ExecutionMode.Primary,
            false);
    }

    private async Task<ChatExecutionResult> ExecuteNonStreamingAsync(
        RouterChatRequest request,
        RouterRequestContext requestContext,
        RouteDecision routeDecision,
        ExecutionMode executionMode,
        bool fallbackUsed,
        CancellationToken cancellationToken)
    {
        UpdateContext(requestContext, routeDecision);
        var stopwatch = Stopwatch.StartNew();
        using var scope = _logger.BeginScope(CreateLogScope(requestContext, routeDecision, request.Stream, executionMode, fallbackUsed));

        try
        {
            var backend = _backendRegistry.GetBackend(routeDecision.SelectedBackend);
            var response = await backend.CompleteChatAsync(request, routeDecision, requestContext, cancellationToken);

            _logger.LogInformation(
                "Chat completion succeeded in {DurationMs} ms. Success={Success}",
                stopwatch.ElapsedMilliseconds,
                true);

            return new ChatExecutionResult(routeDecision, response, executionMode, fallbackUsed);
        }
        catch (Exception ex) when (TryResolveFallback(ex, routeDecision, out var fallbackDecision))
        {
            _logger.LogWarning(
                ex,
                "Primary execution failed for backend {BackendId}. Attempting fallback alias {FallbackAlias}.",
                routeDecision.SelectedBackend,
                fallbackDecision.SelectedAlias);

            requestContext.Metadata["fallback.from"] = routeDecision.SelectedAlias;
            return await ExecuteNonStreamingAsync(
                request,
                requestContext,
                fallbackDecision,
                ExecutionMode.Fallback,
                true,
                cancellationToken);
        }
        catch (Exception ex)
        {
            _logger.LogError(
                ex,
                "Chat completion failed in {DurationMs} ms. Success={Success} ErrorCategory={ErrorCategory}",
                stopwatch.ElapsedMilliseconds,
                false,
                CategorizeError(ex));
            throw;
        }
    }

    private async IAsyncEnumerable<RouterStreamEvent> ExecuteStreamingAsync(
        RouterChatRequest request,
        RouterRequestContext requestContext,
        RouteDecision routeDecision,
        ExecutionMode executionMode,
        bool fallbackUsed,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)
    {
        UpdateContext(requestContext, routeDecision);
        var stopwatch = Stopwatch.StartNew();
        using var scope = _logger.BeginScope(CreateLogScope(requestContext, routeDecision, request.Stream, executionMode, fallbackUsed));

        var yieldedAny = false;
        RouteDecision? fallbackDecision = null;
        var backend = _backendRegistry.GetBackend(routeDecision.SelectedBackend);
        await using var enumerator = backend.StreamChatAsync(request, routeDecision, requestContext, cancellationToken)
            .GetAsyncEnumerator(cancellationToken);

        while (true)
        {
            RouterStreamEvent streamEvent;
            try
            {
                if (!await enumerator.MoveNextAsync())
                {
                    break;
                }

                streamEvent = enumerator.Current;
            }
            catch (Exception ex) when (!yieldedAny && TryResolveFallback(ex, routeDecision, out var resolvedFallback))
            {
                requestContext.Metadata["fallback.from"] = routeDecision.SelectedAlias;
                fallbackDecision = resolvedFallback;
                _logger.LogWarning(
                    ex,
                    "Primary streaming execution failed before the first event for backend {BackendId}. Attempting fallback alias {FallbackAlias}.",
                    routeDecision.SelectedBackend,
                    resolvedFallback.SelectedAlias);
                break;
            }
            catch (Exception ex)
            {
                _logger.LogError(
                    ex,
                    "Streaming chat completion failed in {DurationMs} ms. Success={Success} ErrorCategory={ErrorCategory}",
                    stopwatch.ElapsedMilliseconds,
                    false,
                    CategorizeError(ex));
                throw;
            }

            yieldedAny = true;
            yield return streamEvent;
        }

        if (fallbackDecision is not null)
        {
            await foreach (var streamEvent in ExecuteStreamingAsync(
                               request,
                               requestContext,
                               fallbackDecision,
                               ExecutionMode.Fallback,
                               true,
                               cancellationToken))
            {
                yield return streamEvent;
            }

            yield break;
        }

        _logger.LogInformation(
            "Streaming chat completion succeeded in {DurationMs} ms. Success={Success}",
            stopwatch.ElapsedMilliseconds,
            true);
    }

    private bool TryResolveFallback(Exception exception, RouteDecision routeDecision, out RouteDecision fallbackDecision)
    {
        fallbackDecision = null!;

        if (!_routerOptions.EnableFallback ||
            exception is OperationCanceledException ||
            exception is UpstreamProtocolException ||
            string.IsNullOrWhiteSpace(routeDecision.FallbackAlias))
        {
            return false;
        }

        if (exception is not BackendUnavailableException &&
            exception is not HttpRequestException &&
            exception is not TaskCanceledException)
        {
            return false;
        }

        var resolvedFallback = _routeSelector.ResolveFallbackRoute(routeDecision);
        if (resolvedFallback is null)
        {
            return false;
        }

        fallbackDecision = resolvedFallback;
        return true;
    }

    private static void UpdateContext(RouterRequestContext requestContext, RouteDecision routeDecision)
    {
        requestContext.SelectedAlias = routeDecision.SelectedAlias;
        requestContext.SelectedProfile = routeDecision.SelectedProfile;
        requestContext.SelectedBackend = routeDecision.SelectedBackend;
        requestContext.Metadata["upstream.model"] = routeDecision.UpstreamModel;
    }

    private static IReadOnlyDictionary<string, object?> CreateLogScope(
        RouterRequestContext requestContext,
        RouteDecision routeDecision,
        bool streaming,
        ExecutionMode executionMode,
        bool fallbackUsed) =>
        new Dictionary<string, object?>
        {
            ["RequestId"] = requestContext.RequestId,
            ["SelectedAlias"] = routeDecision.SelectedAlias,
            ["SelectedProfile"] = routeDecision.SelectedProfile,
            ["SelectedBackend"] = routeDecision.SelectedBackend,
            ["UpstreamModel"] = routeDecision.UpstreamModel,
            ["Streaming"] = streaming,
            ["ExecutionMode"] = executionMode.ToString(),
            ["FallbackUsed"] = fallbackUsed
        };

    private static string CategorizeError(Exception exception) =>
        exception switch
        {
            BackendUnavailableException => "backend_unavailable",
            RouteNotFoundException => "route_not_found",
            ProfileResolutionException => "profile_resolution",
            UpstreamProtocolException => "upstream_protocol",
            ConfigurationException => "configuration",
            OperationCanceledException => "cancelled",
            _ => "unhandled"
        };
}
