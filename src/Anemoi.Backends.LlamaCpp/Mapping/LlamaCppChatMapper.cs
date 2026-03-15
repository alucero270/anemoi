using Anemoi.Backends.LlamaCpp.Models;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Models;

namespace Anemoi.Backends.LlamaCpp.Mapping;

public static class LlamaCppChatMapper
{
    public static LlamaCppChatRequest MapRequest(RouterChatRequest request, RouteDecision routeDecision) =>
        new()
        {
            Model = routeDecision.UpstreamModel,
            Stream = request.Stream,
            Temperature = request.Temperature ?? routeDecision.Temperature,
            TopP = request.TopP ?? routeDecision.TopP,
            MaxTokens = request.MaxTokens ?? routeDecision.MaxTokens,
            Messages = request.Messages
                .Select(static message => new LlamaCppMessage { Role = message.Role, Content = message.Content })
                .ToArray()
        };

    public static RouterChatResponse MapResponse(LlamaCppChatResponse response, RouteDecision routeDecision)
    {
        if (response.Choices.Count == 0)
        {
            throw new UpstreamProtocolException("llama.cpp returned no choices.");
        }

        var choices = response.Choices.Select(choice =>
        {
            if (choice.Message?.Content is null || choice.Message.Role is null)
            {
                throw new UpstreamProtocolException("llama.cpp returned a choice without a message payload.");
            }

            return new RouterChoice(
                choice.Index,
                new RouterMessage(choice.Message.Role, choice.Message.Content),
                choice.FinishReason);
        }).ToArray();

        return new RouterChatResponse(
            response.Id ?? $"chatcmpl-{Guid.NewGuid():N}",
            response.Object ?? "chat.completion",
            response.Created == 0 ? DateTimeOffset.UtcNow.ToUnixTimeSeconds() : response.Created,
            routeDecision.SelectedAlias,
            choices,
            response.Usage is null
                ? null
                : new RouterUsage(response.Usage.PromptTokens, response.Usage.CompletionTokens, response.Usage.TotalTokens));
    }

    public static RouterStreamEvent MapStreamEvent(LlamaCppChatResponse response, RouteDecision routeDecision)
    {
        var choice = response.Choices.FirstOrDefault()
                     ?? throw new UpstreamProtocolException("llama.cpp returned a stream chunk without choices.");

        return new RouterStreamEvent(
            response.Id ?? $"chatcmpl-{Guid.NewGuid():N}",
            response.Created == 0 ? DateTimeOffset.UtcNow.ToUnixTimeSeconds() : response.Created,
            routeDecision.SelectedAlias,
            choice.Index,
            choice.Delta?.Role,
            choice.Delta?.Content,
            choice.FinishReason,
            false);
    }
}
