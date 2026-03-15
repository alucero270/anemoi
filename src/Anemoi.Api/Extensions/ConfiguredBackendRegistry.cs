using Microsoft.Extensions.Options;
using Anemoi.Backends.LlamaCpp;
using Anemoi.Backends.Ollama;
using Anemoi.Core.Configuration;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Api.Extensions;

public sealed class ConfiguredBackendRegistry : IBackendRegistry
{
    private readonly IReadOnlyDictionary<string, IChatBackend> _backends;

    public ConfiguredBackendRegistry(
        IServiceProvider serviceProvider,
        IOptions<RouterOptions> options,
        ILogger<ConfiguredBackendRegistry> logger)
    {
        _backends = BuildBackends(serviceProvider, options.Value)
            .ToDictionary(static backend => backend.Descriptor.Id, StringComparer.OrdinalIgnoreCase);

        logger.LogInformation("Registered {BackendCount} enabled backends.", _backends.Count);
    }

    public IChatBackend GetBackend(string backendId)
    {
        if (!_backends.TryGetValue(backendId, out var backend))
        {
            throw new Anemoi.Core.Exceptions.BackendUnavailableException(
                $"Backend '{backendId}' is not registered or not enabled.");
        }

        return backend;
    }

    public IReadOnlyCollection<IChatBackend> GetAllBackends() => _backends.Values.ToArray();

    private static IEnumerable<IChatBackend> BuildBackends(IServiceProvider serviceProvider, RouterOptions options)
    {
        foreach (var backend in options.Backends.Where(static backend => backend.Enabled))
        {
            var descriptor = new BackendDescriptor(
                backend.Id,
                backend.Type,
                new Uri(backend.BaseUrl, UriKind.Absolute),
                TimeSpan.FromSeconds(backend.TimeoutSeconds),
                backend.Enabled,
                backend.AllowInsecureTls,
                new Dictionary<string, string>(backend.Metadata, StringComparer.OrdinalIgnoreCase));

            yield return descriptor.Type switch
            {
                BackendType.Ollama => ActivatorUtilities.CreateInstance<OllamaChatBackend>(serviceProvider, descriptor),
                BackendType.LlamaCpp => ActivatorUtilities.CreateInstance<LlamaCppChatBackend>(serviceProvider, descriptor),
                BackendType.FoundryLocal => throw new InvalidOperationException(
                    "FoundryLocal is reserved for a future stage and cannot be enabled in Stage 1."),
                _ => throw new InvalidOperationException($"Unsupported backend type '{descriptor.Type}'.")
            };
        }
    }
}
