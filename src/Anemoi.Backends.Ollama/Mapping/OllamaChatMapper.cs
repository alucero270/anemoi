using Anemoi.Backends.Ollama.Models;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Models;

namespace Anemoi.Backends.Ollama.Mapping;

public static class OllamaChatMapper
{
    public static OllamaChatRequest MapRequest(RouterChatRequest request, RouteDecision routeDecision) =>
        new()
        {
            Model = routeDecision.UpstreamModel,
            Stream = request.Stream,
            Think = false,
            Messages = request.Messages
                .Select(static message => new OllamaMessage { Role = message.Role, Content = message.Content })
                .ToArray(),
            Options = new OllamaOptions
            {
                Temperature = request.Temperature ?? routeDecision.Temperature,
                TopP = request.TopP ?? routeDecision.TopP,
                NumPredict = request.MaxTokens ?? routeDecision.MaxTokens
            }
        };

    public static RouterChatResponse MapResponse(OllamaChatResponse response, RouteDecision routeDecision)
    {
        if (response.Message?.Content is null)
        {
            throw new UpstreamProtocolException("Ollama response did not include an assistant message.");
        }

        var createdAt = ParseCreatedAt(response.CreatedAt);
        var responseId = $"chatcmpl-{Guid.NewGuid():N}";

        return new RouterChatResponse(
            responseId,
            "chat.completion",
            createdAt,
            routeDecision.SelectedAlias,
            [
                new RouterChoice(
                    0,
                    new RouterMessage(response.Message.Role, response.Message.Content),
                    response.DoneReason ?? "stop")
            ],
            new RouterUsage(
                response.PromptEvalCount ?? 0,
                response.EvalCount ?? 0,
                (response.PromptEvalCount ?? 0) + (response.EvalCount ?? 0)));
    }

    public static RouterStreamEvent? MapStreamEvent(
        OllamaChatResponse response,
        string responseId,
        RouteDecision routeDecision,
        bool firstChunk)
    {
        if (!string.IsNullOrWhiteSpace(response.Message?.Content))
        {
            return new RouterStreamEvent(
                responseId,
                ParseCreatedAt(response.CreatedAt),
                routeDecision.SelectedAlias,
                0,
                firstChunk ? "assistant" : null,
                response.Message.Content,
                null,
                false);
        }

        if (response.Done)
        {
            return new RouterStreamEvent(
                responseId,
                ParseCreatedAt(response.CreatedAt),
                routeDecision.SelectedAlias,
                0,
                null,
                null,
                response.DoneReason ?? "stop",
                false);
        }

        return null;
    }

    private static long ParseCreatedAt(string? createdAt)
    {
        if (createdAt is not null && DateTimeOffset.TryParse(createdAt, out var parsed))
        {
            return parsed.ToUnixTimeSeconds();
        }

        return DateTimeOffset.UtcNow.ToUnixTimeSeconds();
    }
}
