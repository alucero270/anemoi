using Microsoft.Extensions.Logging;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Interfaces;

namespace Anemoi.Core.Services;

public sealed class BackendRegistry : IBackendRegistry
{
    private readonly IReadOnlyDictionary<string, IChatBackend> _backends;

    public BackendRegistry(IEnumerable<IChatBackend> backends, ILogger<BackendRegistry> logger)
    {
        _backends = backends.ToDictionary(static backend => backend.Descriptor.Id, StringComparer.OrdinalIgnoreCase);
        logger.LogInformation("Registered {BackendCount} enabled backends.", _backends.Count);
    }

    public IChatBackend GetBackend(string backendId)
    {
        if (!_backends.TryGetValue(backendId, out var backend))
        {
            throw new BackendUnavailableException($"Backend '{backendId}' is not registered or not enabled.");
        }

        return backend;
    }

    public IReadOnlyCollection<IChatBackend> GetAllBackends() => _backends.Values.ToArray();
}
