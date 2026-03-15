using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Anemoi.Core.Interfaces;

namespace Anemoi.Api.Controllers;

[ApiController]
[Route("health")]
public sealed class HealthController : ControllerBase
{
    private readonly HealthCheckService _healthCheckService;
    private readonly IBackendHealthService _backendHealthService;

    public HealthController(HealthCheckService healthCheckService, IBackendHealthService backendHealthService)
    {
        _healthCheckService = healthCheckService;
        _backendHealthService = backendHealthService;
    }

    [HttpGet]
    public async Task<IActionResult> GetAsync(CancellationToken cancellationToken)
    {
        var report = await _healthCheckService.CheckHealthAsync(cancellationToken);
        var response = new
        {
            status = report.Status.ToString(),
            service = "anemoi-router",
            timestampUtc = DateTimeOffset.UtcNow,
            checks = report.Entries.Select(entry => new
            {
                name = entry.Key,
                status = entry.Value.Status.ToString(),
                duration = entry.Value.Duration.TotalMilliseconds
            })
        };

        return report.Status == HealthStatus.Healthy ? Ok(response) : StatusCode(StatusCodes.Status503ServiceUnavailable, response);
    }

    [HttpGet("backends")]
    public async Task<IActionResult> GetBackendsAsync(CancellationToken cancellationToken)
    {
        var results = await _backendHealthService.GetBackendHealthAsync(cancellationToken);
        var statusCode = results.All(static result => result.IsHealthy)
            ? StatusCodes.Status200OK
            : StatusCodes.Status503ServiceUnavailable;

        return StatusCode(statusCode, new
        {
            status = statusCode == StatusCodes.Status200OK ? "Healthy" : "Degraded",
            timestampUtc = DateTimeOffset.UtcNow,
            backends = results
        });
    }
}
