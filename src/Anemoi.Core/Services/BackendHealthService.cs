using Microsoft.Extensions.Logging;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Core.Services;

public sealed class BackendHealthService : IBackendHealthService
{
    private readonly IBackendRegistry _backendRegistry;
    private readonly ILogger<BackendHealthService> _logger;

    public BackendHealthService(IBackendRegistry backendRegistry, ILogger<BackendHealthService> logger)
    {
        _backendRegistry = backendRegistry;
        _logger = logger;
    }

    public async Task<IReadOnlyCollection<BackendHealthResult>> GetBackendHealthAsync(CancellationToken cancellationToken)
    {
        var tasks = _backendRegistry.GetAllBackends()
            .Select(backend => backend.CheckHealthAsync(cancellationToken))
            .ToArray();

        var results = await Task.WhenAll(tasks);
        _logger.LogInformation(
            "Collected backend health for {BackendCount} backends with {HealthyCount} healthy.",
            results.Length,
            results.Count(static result => result.IsHealthy));

        return results.OrderBy(static result => result.BackendId, StringComparer.OrdinalIgnoreCase).ToArray();
    }
}
