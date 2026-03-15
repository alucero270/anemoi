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
        services.AddSingleton<IBackendRegistry, ConfiguredBackendRegistry>();
        services.AddSingleton<IChatCompletionService, ChatCompletionService>();
        services.AddSingleton<IBackendHealthService, BackendHealthService>();

        return services;
    }
}
