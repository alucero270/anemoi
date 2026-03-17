using System.Text.Json.Serialization;

namespace Anemoi.Backends.Ollama.Models;

public sealed class OllamaChatRequest
{
    [JsonPropertyName("model")]
    public string Model { get; init; } = string.Empty;

    [JsonPropertyName("messages")]
    public IReadOnlyCollection<OllamaMessage> Messages { get; init; } = [];

    [JsonPropertyName("stream")]
    public bool Stream { get; init; }

    [JsonPropertyName("think")]
    public bool Think { get; init; }

    [JsonPropertyName("options")]
    public OllamaOptions? Options { get; init; }
}

public sealed class OllamaMessage
{
    [JsonPropertyName("role")]
    public string Role { get; init; } = string.Empty;

    [JsonPropertyName("content")]
    public string Content { get; init; } = string.Empty;
}

public sealed class OllamaOptions
{
    [JsonPropertyName("temperature")]
    public double? Temperature { get; init; }

    [JsonPropertyName("top_p")]
    public double? TopP { get; init; }

    [JsonPropertyName("num_predict")]
    public int? NumPredict { get; init; }
}

public sealed class OllamaChatResponse
{
    [JsonPropertyName("model")]
    public string? Model { get; init; }

    [JsonPropertyName("created_at")]
    public string? CreatedAt { get; init; }

    [JsonPropertyName("message")]
    public OllamaMessage? Message { get; init; }

    [JsonPropertyName("done")]
    public bool Done { get; init; }

    [JsonPropertyName("done_reason")]
    public string? DoneReason { get; init; }

    [JsonPropertyName("prompt_eval_count")]
    public int? PromptEvalCount { get; init; }

    [JsonPropertyName("eval_count")]
    public int? EvalCount { get; init; }
}
