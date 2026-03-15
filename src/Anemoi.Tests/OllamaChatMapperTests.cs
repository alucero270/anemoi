using Anemoi.Backends.Ollama.Mapping;
using Anemoi.Backends.Ollama.Models;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Models;

namespace Anemoi.Tests;

public sealed class OllamaChatMapperTests
{
    private static readonly RouteDecision Decision = new(
        "default-chat",
        "default-chat-profile",
        "ollama-main",
        "llama3.1:8b",
        0.7,
        0.95,
        512,
        "default-alias",
        "fast");

    [Fact]
    public void MapRequest_UsesCanonicalRequestAndProfileDefaults()
    {
        var request = new RouterChatRequest(null, [ new RouterMessage("user", "Hello") ], false, null, null, null);

        var payload = OllamaChatMapper.MapRequest(request, Decision);

        Assert.Equal("llama3.1:8b", payload.Model);
        Assert.Equal(0.7, payload.Options!.Temperature);
        Assert.Equal("Hello", payload.Messages.Single().Content);
    }

    [Fact]
    public void MapResponse_NormalizesSuccessfulResponse()
    {
        var response = new OllamaChatResponse
        {
            CreatedAt = DateTimeOffset.UtcNow.ToString("O"),
            DoneReason = "stop",
            Message = new OllamaMessage { Role = "assistant", Content = "Hi" },
            PromptEvalCount = 3,
            EvalCount = 5
        };

        var normalized = OllamaChatMapper.MapResponse(response, Decision);

        Assert.Equal("default-chat", normalized.Model);
        Assert.Equal("Hi", normalized.Choices.Single().Message.Content);
        Assert.Equal(8, normalized.Usage!.TotalTokens);
    }

    [Fact]
    public void MapResponse_ThrowsForMalformedResponse()
    {
        var response = new OllamaChatResponse();

        Assert.Throws<UpstreamProtocolException>(() => OllamaChatMapper.MapResponse(response, Decision));
    }
}
