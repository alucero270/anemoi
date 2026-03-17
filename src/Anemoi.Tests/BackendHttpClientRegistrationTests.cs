using System.Reflection;
using Microsoft.Extensions.DependencyInjection;
using Anemoi.Backends.LlamaCpp.Clients;
using Anemoi.Backends.LlamaCpp.DependencyInjection;
using Anemoi.Backends.Ollama.Clients;
using Anemoi.Backends.Ollama.DependencyInjection;

namespace Anemoi.Tests;

public sealed class BackendHttpClientRegistrationTests
{
    [Fact]
    public void OllamaClient_UsesInfiniteHttpClientTimeout()
    {
        var services = new ServiceCollection();
        services.AddLogging();
        services.AddOllamaBackendSupport();

        using var provider = services.BuildServiceProvider();
        var client = provider.GetRequiredService<OllamaHttpClient>();

        Assert.Equal(Timeout.InfiniteTimeSpan, GetInnerHttpClient(client).Timeout);
    }

    [Fact]
    public void LlamaCppClient_UsesInfiniteHttpClientTimeout()
    {
        var services = new ServiceCollection();
        services.AddLogging();
        services.AddLlamaCppBackendSupport();

        using var provider = services.BuildServiceProvider();
        var client = provider.GetRequiredService<LlamaCppHttpClient>();

        Assert.Equal(Timeout.InfiniteTimeSpan, GetInnerHttpClient(client).Timeout);
    }

    private static HttpClient GetInnerHttpClient(object typedClient)
    {
        var field = typedClient.GetType().GetField("_httpClient", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?? throw new InvalidOperationException("Typed client did not expose the expected HttpClient field.");

        return (HttpClient)(field.GetValue(typedClient)
                            ?? throw new InvalidOperationException("Typed client did not contain an HttpClient instance."));
    }
}
