using Microsoft.Extensions.DependencyInjection;

namespace Anemoi.Backends.Ollama.DependencyInjection;

public static class ServiceCollectionExtensions
{
    public static IServiceCollection AddOllamaBackendSupport(this IServiceCollection services)
    {
        services.AddHttpClient<Clients.OllamaHttpClient>();
        return services;
    }
}
