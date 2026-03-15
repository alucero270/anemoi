using System.Net;
using System.Text;
using System.Text.Json;
using Microsoft.Extensions.Logging;
using Anemoi.Backends.Ollama.Clients;
using Anemoi.Backends.Ollama.Mapping;
using Anemoi.Backends.Ollama.Models;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Backends.Ollama;

public sealed class OllamaChatBackend : IChatBackend
{
    private static readonly JsonSerializerOptions SerializerOptions = new(JsonSerializerDefaults.Web);
    private readonly OllamaHttpClient _httpClient;
    private readonly ILogger<OllamaChatBackend> _logger;

    public OllamaChatBackend(BackendDescriptor descriptor, OllamaHttpClient httpClient, ILogger<OllamaChatBackend> logger)
    {
        Descriptor = descriptor;
        _httpClient = httpClient;
        _logger = logger;
    }

    public BackendDescriptor Descriptor { get; }

    public async Task<RouterChatResponse> CompleteChatAsync(
        RouterChatRequest request,
        RouteDecision decision,
        RouterRequestContext requestContext,
        CancellationToken cancellationToken)
    {
        var payload = OllamaChatMapper.MapRequest(request, decision);
        using var httpRequest = CreateChatRequest(payload);
        using var response = await SendChatRequestAsync(httpRequest, HttpCompletionOption.ResponseContentRead, cancellationToken);
        var responseContent = await response.Content.ReadAsStringAsync(cancellationToken);

        try
        {
            var parsed = JsonSerializer.Deserialize<OllamaChatResponse>(responseContent, SerializerOptions)
                         ?? throw new UpstreamProtocolException("Ollama returned an empty response payload.");

            return OllamaChatMapper.MapResponse(parsed, decision);
        }
        catch (JsonException ex)
        {
            throw new UpstreamProtocolException("Ollama returned malformed JSON.", ex);
        }
    }

    public async IAsyncEnumerable<RouterStreamEvent> StreamChatAsync(
        RouterChatRequest request,
        RouteDecision decision,
        RouterRequestContext requestContext,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)
    {
        var payload = OllamaChatMapper.MapRequest(request with { Stream = true }, decision);
        using var httpRequest = CreateChatRequest(payload);
        using var response = await SendChatRequestAsync(httpRequest, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        await using var responseStream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var reader = new StreamReader(responseStream);

        var responseId = $"chatcmpl-{Guid.NewGuid():N}";
        var firstChunk = true;

        while (true)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var line = await reader.ReadLineAsync(cancellationToken);
            if (line is null)
            {
                break;
            }

            if (string.IsNullOrWhiteSpace(line))
            {
                continue;
            }

            OllamaChatResponse parsed;
            try
            {
                parsed = JsonSerializer.Deserialize<OllamaChatResponse>(line, SerializerOptions)
                         ?? throw new UpstreamProtocolException("Ollama returned an empty stream event.");
            }
            catch (JsonException ex)
            {
                throw new UpstreamProtocolException("Ollama returned malformed streaming JSON.", ex);
            }

            var streamEvent = OllamaChatMapper.MapStreamEvent(parsed, responseId, decision, firstChunk);
            if (streamEvent is not null)
            {
                firstChunk = false;
                yield return streamEvent;
            }
        }
    }

    public async Task<BackendHealthResult> CheckHealthAsync(CancellationToken cancellationToken)
    {
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, "/api/tags");
            using var response = await _httpClient.SendAsync(Descriptor, request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);

            return new BackendHealthResult(
                Descriptor.Id,
                Descriptor.Type,
                response.IsSuccessStatusCode,
                response.IsSuccessStatusCode ? "Healthy" : $"HTTP {(int)response.StatusCode}",
                DateTimeOffset.UtcNow,
                response.IsSuccessStatusCode ? null : "Ollama health probe returned a non-success status code.");
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Health probe failed for backend {BackendId}.", Descriptor.Id);
            return new BackendHealthResult(Descriptor.Id, Descriptor.Type, false, "Unhealthy", DateTimeOffset.UtcNow, ex.Message);
        }
    }

    private HttpRequestMessage CreateChatRequest(OllamaChatRequest payload)
    {
        var json = JsonSerializer.Serialize(payload, SerializerOptions);
        return new HttpRequestMessage(HttpMethod.Post, "/api/chat")
        {
            Content = new StringContent(json, Encoding.UTF8, "application/json")
        };
    }

    private async Task<HttpResponseMessage> SendChatRequestAsync(
        HttpRequestMessage request,
        HttpCompletionOption completionOption,
        CancellationToken cancellationToken)
    {
        try
        {
            var response = await _httpClient.SendAsync(Descriptor, request, completionOption, cancellationToken);
            if (response.IsSuccessStatusCode)
            {
                return response;
            }

            var message = $"Ollama backend '{Descriptor.Id}' returned HTTP {(int)response.StatusCode}.";
            if (response.StatusCode == HttpStatusCode.BadRequest)
            {
                throw new UpstreamProtocolException(message);
            }

            throw new BackendUnavailableException(message);
        }
        catch (UpstreamProtocolException)
        {
            throw;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException)
        {
            throw new BackendUnavailableException($"Ollama backend '{Descriptor.Id}' is unavailable.", ex);
        }
    }
}
