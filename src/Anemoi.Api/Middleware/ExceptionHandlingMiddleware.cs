using Microsoft.AspNetCore.Mvc;
using Anemoi.Core.Exceptions;

namespace Anemoi.Api.Middleware;

public sealed class ExceptionHandlingMiddleware
{
    private readonly RequestDelegate _next;
    private readonly ILogger<ExceptionHandlingMiddleware> _logger;

    public ExceptionHandlingMiddleware(RequestDelegate next, ILogger<ExceptionHandlingMiddleware> logger)
    {
        _next = next;
        _logger = logger;
    }

    public async Task InvokeAsync(HttpContext context)
    {
        try
        {
            await _next(context);
        }
        catch (Exception ex)
        {
            await HandleExceptionAsync(context, ex);
        }
    }

    private async Task HandleExceptionAsync(HttpContext context, Exception exception)
    {
        var (statusCode, title) = exception switch
        {
            ArgumentException => (StatusCodes.Status400BadRequest, "Bad Request"),
            RouteNotFoundException => (StatusCodes.Status404NotFound, "Route Not Found"),
            ProfileResolutionException => (StatusCodes.Status400BadRequest, "Profile Resolution Failed"),
            BackendUnavailableException => (StatusCodes.Status503ServiceUnavailable, "Backend Unavailable"),
            UpstreamProtocolException => (StatusCodes.Status502BadGateway, "Upstream Protocol Error"),
            ConfigurationException => (StatusCodes.Status500InternalServerError, "Configuration Error"),
            _ => (StatusCodes.Status500InternalServerError, "Unhandled Error")
        };

        _logger.LogError(exception, "Request failed with HTTP {StatusCode}.", statusCode);

        context.Response.StatusCode = statusCode;
        context.Response.ContentType = "application/problem+json";

        var requestId = context.Response.Headers.TryGetValue("x-request-id", out var headerValue)
            ? headerValue.ToString()
            : context.TraceIdentifier;

        var problem = new ProblemDetails
        {
            Status = statusCode,
            Title = title,
            Detail = exception.Message,
            Instance = context.Request.Path
        };

        problem.Extensions["requestId"] = requestId;
        await context.Response.WriteAsJsonAsync(problem);
    }
}
