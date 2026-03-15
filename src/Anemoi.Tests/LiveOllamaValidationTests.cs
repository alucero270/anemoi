using System.Net;
using System.Net.Http.Json;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;
using Anemoi.Api.Models;
using Anemoi.Core.Models;

namespace Anemoi.Tests;

public sealed class LiveOllamaValidationTests
{
    [Fact]
    public async Task LiveOllamaRouterValidation_WorksEndToEnd()
    {
        var ollamaUrl = Environment.GetEnvironmentVariable("ANEMOI_LIVE_OLLAMA_URL");
        var ollamaModel = Environment.GetEnvironmentVariable("ANEMOI_LIVE_OLLAMA_MODEL");

        if (string.IsNullOrWhiteSpace(ollamaUrl) || string.IsNullOrWhiteSpace(ollamaModel))
        {
            return;
        }

        using var factory = new LiveOllamaApiFactory(ollamaUrl, ollamaModel);
        using var client = factory.CreateClient();

        using var healthResponse = await client.GetAsync("/health/backends");
        var healthBody = await healthResponse.Content.ReadAsStringAsync();
        Assert.True(
            healthResponse.StatusCode == HttpStatusCode.OK,
            $"Expected OK from /health/backends but received {(int)healthResponse.StatusCode}: {healthBody}");
        Assert.Contains("ollama-main", healthBody);
        Assert.DoesNotContain("Unhealthy", healthBody, StringComparison.OrdinalIgnoreCase);

        var chatRequest = new ChatCompletionRequestDto
        {
            Model = "default-chat",
            Messages = [ new ChatMessageDto { Role = "user", Content = "Reply with the single word READY." } ]
        };

        using var chatResponse = await client.PostAsJsonAsync("/v1/chat/completions", chatRequest);
        var chatPayload = await chatResponse.Content.ReadFromJsonAsync<ChatCompletionResponseDto>();
        Assert.Equal(HttpStatusCode.OK, chatResponse.StatusCode);
        Assert.NotNull(chatPayload);
        Assert.False(string.IsNullOrWhiteSpace(chatPayload!.Choices.Single().Message.Content));
        Assert.True(chatResponse.Headers.Contains("x-request-id"));

        using var streamingRequest = new HttpRequestMessage(HttpMethod.Post, "/v1/chat/completions")
        {
            Content = JsonContent.Create(new
            {
                model = "default-chat",
                stream = true,
                messages = new[] { new { role = "user", content = "Reply with the single word STREAM." } }
            })
        };

        using var streamingResponse = await client.SendAsync(streamingRequest, HttpCompletionOption.ResponseHeadersRead);
        var streamingBody = await streamingResponse.Content.ReadAsStringAsync();
        Assert.Equal(HttpStatusCode.OK, streamingResponse.StatusCode);
        Assert.Contains("data:", streamingBody);
        Assert.Contains("[DONE]", streamingBody);
    }

    private sealed class LiveOllamaApiFactory : WebApplicationFactory<Program>
    {
        private readonly string _ollamaUrl;
        private readonly string _ollamaModel;

        public LiveOllamaApiFactory(string ollamaUrl, string ollamaModel)
        {
            _ollamaUrl = ollamaUrl;
            _ollamaModel = ollamaModel;
        }

        protected override void ConfigureWebHost(IWebHostBuilder builder)
        {
            builder.UseEnvironment("Testing");
            builder.ConfigureAppConfiguration((_, config) =>
            {
                var options = TestConfiguration.CreateRouterOptions();
                options.EnableFallback = false;
                options.Backends[0].BaseUrl = _ollamaUrl;
                options.Backends[0].AllowInsecureTls = true;
                options.Backends[1].Enabled = false;

                foreach (var profile in options.Profiles)
                {
                    profile.BackendId = "ollama-main";
                    profile.UpstreamModel = _ollamaModel;
                    profile.ExecutionTarget = ExecutionTarget.Local;
                }

                config.Sources.Clear();
                config.AddInMemoryCollection(TestConfiguration.ToDictionary(options));
            });
        }
    }
}
