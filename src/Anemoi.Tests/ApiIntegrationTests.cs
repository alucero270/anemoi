using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Anemoi.Backends.LlamaCpp.Clients;
using Anemoi.Backends.Ollama.Clients;
using Anemoi.Api.Models;

namespace Anemoi.Tests;

public sealed class ApiIntegrationTests
{
    [Fact]
    public async Task ChatCompletion_ReturnsSuccessfulResponse()
    {
        using var factory = new AnemoiApiFactory(
            ollamaResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/api/chat" => JsonResponse(new
                {
                    model = "llama3.1:8b",
                    created_at = DateTimeOffset.UtcNow.ToString("O"),
                    message = new { role = "assistant", content = "Hello from Ollama" },
                    done = true,
                    done_reason = "stop",
                    prompt_eval_count = 4,
                    eval_count = 5
                }),
                "/api/tags" => JsonResponse(new { models = Array.Empty<object>() }),
                _ => new HttpResponseMessage(HttpStatusCode.NotFound)
            },
            llamaCppResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/health" => new HttpResponseMessage(HttpStatusCode.OK),
                _ => new HttpResponseMessage(HttpStatusCode.OK)
            });

        using var client = factory.CreateClient();
        var request = new ChatCompletionRequestDto
        {
            Model = "default-chat",
            Messages = [ new ChatMessageDto { Role = "user", Content = "hello" } ]
        };

        using var response = await client.PostAsJsonAsync("/v1/chat/completions", request);
        var payload = await response.Content.ReadFromJsonAsync<ChatCompletionResponseDto>();

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.NotNull(payload);
        Assert.Equal("default-chat", payload!.Model);
        Assert.Equal("Hello from Ollama", payload.Choices.Single().Message.Content);
    }

    [Fact]
    public async Task StreamingChatCompletion_ReturnsEventStream()
    {
        using var factory = new AnemoiApiFactory(
            ollamaResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/api/chat" => new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new StringContent(
                        "{\"model\":\"llama3.1:8b\",\"created_at\":\"2026-03-15T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"content\":\"Hel\"},\"done\":false}\n" +
                        "{\"model\":\"llama3.1:8b\",\"created_at\":\"2026-03-15T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"content\":\"lo\"},\"done\":false}\n" +
                        "{\"model\":\"llama3.1:8b\",\"created_at\":\"2026-03-15T00:00:02Z\",\"done\":true,\"done_reason\":\"stop\"}\n",
                        Encoding.UTF8,
                        "application/x-ndjson")
                },
                "/api/tags" => JsonResponse(new { models = Array.Empty<object>() }),
                _ => new HttpResponseMessage(HttpStatusCode.NotFound)
            },
            llamaCppResponder: _ => new HttpResponseMessage(HttpStatusCode.OK));

        using var client = factory.CreateClient();
        using var request = new HttpRequestMessage(HttpMethod.Post, "/v1/chat/completions")
        {
            Content = JsonContent.Create(new
            {
                model = "default-chat",
                stream = true,
                messages = new[] { new { role = "user", content = "hello" } }
            })
        };

        using var response = await client.SendAsync(request, HttpCompletionOption.ResponseHeadersRead);
        var body = await response.Content.ReadAsStringAsync();

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.Contains("data:", body);
        Assert.Contains("[DONE]", body);
    }

    [Fact]
    public async Task ChatCompletion_UsesFallback_WhenPrimaryBackendFails()
    {
        using var factory = new AnemoiApiFactory(
            ollamaResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/api/chat" => new HttpResponseMessage(HttpStatusCode.ServiceUnavailable),
                "/api/tags" => JsonResponse(new { models = Array.Empty<object>() }),
                _ => new HttpResponseMessage(HttpStatusCode.NotFound)
            },
            llamaCppResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/v1/chat/completions" => JsonResponse(new
                {
                    id = "chatcmpl-fallback",
                    @object = "chat.completion",
                    created = DateTimeOffset.UtcNow.ToUnixTimeSeconds(),
                    choices = new[]
                    {
                        new
                        {
                            index = 0,
                            message = new { role = "assistant", content = "Fallback response" },
                            finish_reason = "stop"
                        }
                    },
                    usage = new { prompt_tokens = 3, completion_tokens = 2, total_tokens = 5 }
                }),
                "/health" => new HttpResponseMessage(HttpStatusCode.OK),
                _ => new HttpResponseMessage(HttpStatusCode.NotFound)
            });

        using var client = factory.CreateClient();
        var request = new ChatCompletionRequestDto
        {
            Model = "default-chat",
            Messages = [ new ChatMessageDto { Role = "user", Content = "hello" } ]
        };

        using var response = await client.PostAsJsonAsync("/v1/chat/completions", request);
        var payload = await response.Content.ReadFromJsonAsync<ChatCompletionResponseDto>();

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.Equal("fast", payload!.Model);
        Assert.Equal("Fallback response", payload.Choices.Single().Message.Content);
    }

    [Fact]
    public async Task HealthAndModelsEndpoints_ReturnExpectedPayloads()
    {
        using var factory = new AnemoiApiFactory(
            ollamaResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/api/tags" => JsonResponse(new { models = Array.Empty<object>() }),
                "/api/chat" => JsonResponse(new { }),
                _ => new HttpResponseMessage(HttpStatusCode.NotFound)
            },
            llamaCppResponder: request => request.RequestUri!.AbsolutePath switch
            {
                "/health" => new HttpResponseMessage(HttpStatusCode.OK),
                _ => new HttpResponseMessage(HttpStatusCode.OK)
            });

        using var client = factory.CreateClient();
        using var healthResponse = await client.GetAsync("/health/backends");
        using var modelsResponse = await client.GetAsync("/v1/models");
        var modelsPayload = await modelsResponse.Content.ReadFromJsonAsync<ModelListResponseDto>();

        Assert.Equal(HttpStatusCode.OK, healthResponse.StatusCode);
        Assert.Equal(HttpStatusCode.OK, modelsResponse.StatusCode);
        Assert.NotNull(modelsPayload);
        Assert.Contains(modelsPayload!.Data, model => model.Id == "default-chat");
        Assert.Contains(modelsPayload.Data, model => model.Id == "code");
    }

    private static HttpResponseMessage JsonResponse(object payload) =>
        new(HttpStatusCode.OK)
        {
            Content = new StringContent(JsonSerializer.Serialize(payload), Encoding.UTF8, "application/json")
        };

    private sealed class AnemoiApiFactory : WebApplicationFactory<Program>
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> _ollamaResponder;
        private readonly Func<HttpRequestMessage, HttpResponseMessage> _llamaCppResponder;

        public AnemoiApiFactory(
            Func<HttpRequestMessage, HttpResponseMessage> ollamaResponder,
            Func<HttpRequestMessage, HttpResponseMessage> llamaCppResponder)
        {
            _ollamaResponder = ollamaResponder;
            _llamaCppResponder = llamaCppResponder;
        }

        protected override void ConfigureWebHost(Microsoft.AspNetCore.Hosting.IWebHostBuilder builder)
        {
            builder.UseEnvironment("Testing");
            builder.ConfigureAppConfiguration((_, config) =>
            {
                config.Sources.Clear();
                config.AddInMemoryCollection(TestConfiguration.ToDictionary(TestConfiguration.CreateRouterOptions()));
            });
            builder.ConfigureTestServices(services =>
            {
                services.RemoveAll<OllamaHttpClient>();
                services.RemoveAll<LlamaCppHttpClient>();

                services.AddHttpClient<OllamaHttpClient>()
                    .ConfigurePrimaryHttpMessageHandler(() => new DelegateHttpMessageHandler(_ollamaResponder));
                services.AddHttpClient<LlamaCppHttpClient>()
                    .ConfigurePrimaryHttpMessageHandler(() => new DelegateHttpMessageHandler(_llamaCppResponder));
            });
        }
    }

    private sealed class DelegateHttpMessageHandler : HttpMessageHandler
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> _responder;

        public DelegateHttpMessageHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        {
            _responder = responder;
        }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) =>
            Task.FromResult(_responder(request));
    }
}
