using System.Net;
using System.Text;
using System.Text.Json;
using Microsoft.Extensions.Logging;
using Anemoi.Backends.LlamaCpp.Clients;
using Anemoi.Backends.LlamaCpp.Mapping;
using Anemoi.Backends.LlamaCpp.Models;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Backends.LlamaCpp;

public sealed class LlamaCppChatBackend : IChatBackend
{
    private static readonly JsonSerializerOptions SerializerOptions = new(JsonSerializerDefaults.Web);
    private readonly LlamaCppHttpClient _httpClient;
    private readonly ILogger<LlamaCppChatBackend> _logger;

    public LlamaCppChatBackend(BackendDescriptor descriptor, LlamaCppHttpClient httpClient, ILogger<LlamaCppChatBackend> logger)
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
        var payload = LlamaCppChatMapper.MapRequest(request, decision);
        using var httpRequest = CreateChatRequest(payload);
        using var response = await SendChatRequestAsync(httpRequest, HttpCompletionOption.ResponseContentRead, cancellationToken);
        var responseContent = await response.Content.ReadAsStringAsync(cancellationToken);

        try
        {
            var parsed = JsonSerializer.Deserialize<LlamaCppChatResponse>(responseContent, SerializerOptions)
                         ?? throw new UpstreamProtocolException("llama.cpp returned an empty response payload.");

            return LlamaCppChatMapper.MapResponse(parsed, decision);
        }
        catch (JsonException ex)
        {
            throw new UpstreamProtocolException("llama.cpp returned malformed JSON.", ex);
        }
    }

    public async IAsyncEnumerable<RouterStreamEvent> StreamChatAsync(
        RouterChatRequest request,
        RouteDecision decision,
        RouterRequestContext requestContext,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)
    {
        var payload = LlamaCppChatMapper.MapRequest(request with { Stream = true }, decision);
        using var httpRequest = CreateChatRequest(payload);
        using var response = await SendChatRequestAsync(httpRequest, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        await using var responseStream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var reader = new StreamReader(responseStream);

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

            if (!line.StartsWith("data:", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            var payloadLine = line["data:".Length..].Trim();
            if (string.Equals(payloadLine, "[DONE]", StringComparison.Ordinal))
            {
                yield break;
            }

            LlamaCppChatResponse parsed;
            try
            {
                parsed = JsonSerializer.Deserialize<LlamaCppChatResponse>(payloadLine, SerializerOptions)
                         ?? throw new UpstreamProtocolException("llama.cpp returned an empty stream event.");
            }
            catch (JsonException ex)
            {
                throw new UpstreamProtocolException("llama.cpp returned malformed streaming JSON.", ex);
            }

            yield return LlamaCppChatMapper.MapStreamEvent(parsed, decision);
        }
    }

    public async Task<BackendHealthResult> CheckHealthAsync(CancellationToken cancellationToken)
    {
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, "/health");
            using var response = await _httpClient.SendAsync(Descriptor, request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);

            return new BackendHealthResult(
                Descriptor.Id,
                Descriptor.Type,
                response.IsSuccessStatusCode,
                response.IsSuccessStatusCode ? "Healthy" : $"HTTP {(int)response.StatusCode}",
                DateTimeOffset.UtcNow,
                response.IsSuccessStatusCode ? null : "llama.cpp health probe returned a non-success status code.");
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Health probe failed for backend {BackendId}.", Descriptor.Id);
            return new BackendHealthResult(Descriptor.Id, Descriptor.Type, false, "Unhealthy", DateTimeOffset.UtcNow, ex.Message);
        }
    }

    private HttpRequestMessage CreateChatRequest(LlamaCppChatRequest payload)
    {
        var json = JsonSerializer.Serialize(payload, SerializerOptions);
        return new HttpRequestMessage(HttpMethod.Post, "/v1/chat/completions")
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

            var message = $"llama.cpp backend '{Descriptor.Id}' returned HTTP {(int)response.StatusCode}.";
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
            throw new BackendUnavailableException($"llama.cpp backend '{Descriptor.Id}' is unavailable.", ex);
        }
    }
}
