using Anemoi.Backends.LlamaCpp.Mapping;
using Anemoi.Backends.LlamaCpp.Models;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Models;

namespace Anemoi.Tests;

public sealed class LlamaCppChatMapperTests
{
    private static readonly RouteDecision Decision = new(
        "code",
        "code-profile",
        "llamacpp-main",
        "qwen2.5-coder",
        0.2,
        0.9,
        1024,
        "explicit-alias",
        "default-chat");

    [Fact]
    public void MapRequest_UsesCanonicalRequestAndProfileDefaults()
    {
        var request = new RouterChatRequest(null, [ new RouterMessage("user", "Write code") ], false, null, null, null);

        var payload = LlamaCppChatMapper.MapRequest(request, Decision);

        Assert.Equal("qwen2.5-coder", payload.Model);
        Assert.Equal(1024, payload.MaxTokens);
        Assert.Equal("Write code", payload.Messages.Single().Content);
    }

    [Fact]
    public void MapResponse_NormalizesOpenAiCompatiblePayload()
    {
        var response = new LlamaCppChatResponse
        {
            Id = "chatcmpl-1",
            Object = "chat.completion",
            Created = DateTimeOffset.UtcNow.ToUnixTimeSeconds(),
            Choices =
            [
                new LlamaCppChoice
                {
                    Index = 0,
                    Message = new LlamaCppMessage { Role = "assistant", Content = "Here is code" },
                    FinishReason = "stop"
                }
            ],
            Usage = new LlamaCppUsage { PromptTokens = 4, CompletionTokens = 6, TotalTokens = 10 }
        };

        var normalized = LlamaCppChatMapper.MapResponse(response, Decision);

        Assert.Equal("code", normalized.Model);
        Assert.Equal("Here is code", normalized.Choices.Single().Message.Content);
        Assert.Equal(10, normalized.Usage!.TotalTokens);
    }

    [Fact]
    public void MapResponse_ThrowsForMalformedPayload()
    {
        var response = new LlamaCppChatResponse();

        Assert.Throws<UpstreamProtocolException>(() => LlamaCppChatMapper.MapResponse(response, Decision));
    }
}
