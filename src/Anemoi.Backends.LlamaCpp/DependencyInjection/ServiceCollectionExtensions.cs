using Microsoft.Extensions.DependencyInjection;

namespace Anemoi.Backends.LlamaCpp.DependencyInjection;

public static class ServiceCollectionExtensions
{
    public static IServiceCollection AddLlamaCppBackendSupport(this IServiceCollection services)
    {
        services.AddHttpClient<Clients.LlamaCppHttpClient>();
        return services;
    }
}
