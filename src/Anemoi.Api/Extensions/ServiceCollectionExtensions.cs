using Microsoft.Extensions.Options;
using Anemoi.Backends.LlamaCpp;
using Anemoi.Backends.LlamaCpp.DependencyInjection;
using Anemoi.Backends.Ollama;
using Anemoi.Backends.Ollama.DependencyInjection;
using Anemoi.Core.Configuration;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;
using Anemoi.Core.Services;

namespace Anemoi.Api.Extensions;

public static class ServiceCollectionExtensions
{
    public static IServiceCollection AddRouterApi(this IServiceCollection services, IConfiguration configuration)
    {
        services.AddControllers()
            .AddJsonOptions(options =>
            {
                options.JsonSerializerOptions.PropertyNamingPolicy = System.Text.Json.JsonNamingPolicy.CamelCase;
                options.JsonSerializerOptions.DefaultIgnoreCondition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull;
            });

        services.AddHealthChecks();
        services.AddOllamaBackendSupport();
        services.AddLlamaCppBackendSupport();

        services.AddSingleton<IValidateOptions<RouterOptions>, RouterOptionsValidator>();
        services.AddOptions<RouterOptions>()
            .Bind(configuration.GetRequiredSection(RouterOptions.SectionName))
            .ValidateOnStart();

        services.AddSingleton<IProfileResolver, ProfileResolver>();
        services.AddSingleton<IRouteSelector, RouteSelector>();
        services.AddSingleton<IBackendRegistry, BackendRegistry>();
        services.AddSingleton<IChatCompletionService, ChatCompletionService>();
        services.AddSingleton<IBackendHealthService, BackendHealthService>();

        var options = configuration.GetSection(RouterOptions.SectionName).Get<RouterOptions>() ?? new RouterOptions();
        foreach (var descriptor in BuildBackendDescriptors(options))
        {
            switch (descriptor.Type)
            {
                case BackendType.Ollama:
                    services.AddSingleton<IChatBackend>(provider => ActivatorUtilities.CreateInstance<OllamaChatBackend>(provider, descriptor));
                    break;
                case BackendType.LlamaCpp:
                    services.AddSingleton<IChatBackend>(provider => ActivatorUtilities.CreateInstance<LlamaCppChatBackend>(provider, descriptor));
                    break;
                case BackendType.FoundryLocal:
                    if (descriptor.Enabled)
                    {
                        throw new InvalidOperationException("FoundryLocal is reserved for a future stage and cannot be enabled in Stage 1.");
                    }

                    break;
                default:
                    throw new InvalidOperationException($"Unsupported backend type '{descriptor.Type}'.");
            }
        }

        return services;
    }

    private static IEnumerable<BackendDescriptor> BuildBackendDescriptors(RouterOptions options) =>
        options.Backends
            .Where(static backend => backend.Enabled)
            .Select(static backend => new BackendDescriptor(
                backend.Id,
                backend.Type,
                new Uri(backend.BaseUrl, UriKind.Absolute),
                TimeSpan.FromSeconds(backend.TimeoutSeconds),
                backend.Enabled,
                new Dictionary<string, string>(backend.Metadata, StringComparer.OrdinalIgnoreCase)));
}
