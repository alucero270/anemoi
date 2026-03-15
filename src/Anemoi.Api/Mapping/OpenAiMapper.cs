using Anemoi.Api.Models;
using Anemoi.Core.Models;

namespace Anemoi.Api.Mapping;

public static class OpenAiMapper
{
    public static RouterChatRequest ToRouterRequest(ChatCompletionRequestDto request) =>
        new(
            request.Model,
            request.Messages.Select(static message => new RouterMessage(message.Role, message.Content, message.Name)).ToArray(),
            request.Stream,
            request.Temperature,
            request.TopP,
            request.MaxTokens,
            request.Metadata);

    public static ChatCompletionResponseDto ToChatCompletionResponse(RouterChatResponse response) =>
        new()
        {
            Id = response.ResponseId,
            Object = response.ObjectType,
            Created = response.CreatedAtEpochSeconds,
            Model = response.Model,
            Choices = response.Choices.Select(static choice => new ChatCompletionChoiceDto
            {
                Index = choice.Index,
                Message = new ChatMessageDto
                {
                    Role = choice.Message.Role,
                    Content = choice.Message.Content,
                    Name = choice.Message.Name
                },
                FinishReason = choice.FinishReason
            }).ToArray(),
            Usage = response.Usage is null
                ? null
                : new ChatCompletionUsageDto
                {
                    PromptTokens = response.Usage.PromptTokens,
                    CompletionTokens = response.Usage.CompletionTokens,
                    TotalTokens = response.Usage.TotalTokens
                }
        };

    public static ChatCompletionChunkDto ToChatCompletionChunk(RouterStreamEvent streamEvent) =>
        new()
        {
            Id = streamEvent.ResponseId,
            Created = streamEvent.CreatedAtEpochSeconds,
            Model = streamEvent.Model,
            Choices =
            [
                new ChatCompletionChunkChoiceDto
                {
                    Index = streamEvent.ChoiceIndex,
                    Delta = new ChatCompletionChunkDeltaDto
                    {
                        Role = streamEvent.DeltaRole,
                        Content = streamEvent.DeltaContent
                    },
                    FinishReason = streamEvent.FinishReason
                }
            ]
        };
}
