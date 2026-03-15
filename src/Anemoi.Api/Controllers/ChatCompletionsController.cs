using System.Text.Json;
using Microsoft.AspNetCore.Mvc;
using Anemoi.Api.Mapping;
using Anemoi.Api.Models;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Api.Controllers;

[ApiController]
[Route("v1/chat")]
public sealed class ChatCompletionsController : ControllerBase
{
    private static readonly JsonSerializerOptions SerializerOptions = new(JsonSerializerDefaults.Web)
    {
        DefaultIgnoreCondition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull
    };

    private readonly IChatCompletionService _chatCompletionService;
    private readonly ILogger<ChatCompletionsController> _logger;

    public ChatCompletionsController(
        IChatCompletionService chatCompletionService,
        ILogger<ChatCompletionsController> logger)
    {
        _chatCompletionService = chatCompletionService;
        _logger = logger;
    }

    [HttpPost("completions")]
    public async Task<IActionResult> CreateAsync([FromBody] ChatCompletionRequestDto request, CancellationToken cancellationToken)
    {
        ValidateRequest(request);

        var routerRequest = OpenAiMapper.ToRouterRequest(request);
        var requestContext = CreateRequestContext(request.Model);
        Response.Headers["x-request-id"] = requestContext.RequestId;

        using var scope = _logger.BeginScope(new Dictionary<string, object?> { ["RequestId"] = requestContext.RequestId });

        if (request.Stream)
        {
            var streamingResult = _chatCompletionService.Stream(routerRequest, requestContext, cancellationToken);
            Response.ContentType = "text/event-stream";
            Response.Headers.CacheControl = "no-cache";

            await foreach (var streamEvent in streamingResult.Events.WithCancellation(cancellationToken))
            {
                var chunk = OpenAiMapper.ToChatCompletionChunk(streamEvent);
                var payload = JsonSerializer.Serialize(chunk, SerializerOptions);
                await Response.WriteAsync($"data: {payload}\n\n", cancellationToken);
                await Response.Body.FlushAsync(cancellationToken);
            }

            await Response.WriteAsync("data: [DONE]\n\n", cancellationToken);
            await Response.Body.FlushAsync(cancellationToken);
            return new EmptyResult();
        }

        var result = await _chatCompletionService.CompleteAsync(routerRequest, requestContext, cancellationToken);
        return Ok(OpenAiMapper.ToChatCompletionResponse(result.Response));
    }

    private static void ValidateRequest(ChatCompletionRequestDto request)
    {
        if (request.Messages.Count == 0)
        {
            throw new ArgumentException("At least one message is required.");
        }

        if (request.Messages.Any(static message => string.IsNullOrWhiteSpace(message.Role) || string.IsNullOrWhiteSpace(message.Content)))
        {
            throw new ArgumentException("Each message requires both role and content.");
        }
    }

    private RouterRequestContext CreateRequestContext(string? requestedModel)
    {
        var requestId = Request.Headers.TryGetValue("x-request-id", out var headerValue) && !string.IsNullOrWhiteSpace(headerValue)
            ? headerValue.ToString()
            : HttpContext.TraceIdentifier;

        var context = new RouterRequestContext
        {
            RequestId = requestId,
            StartedAtUtc = DateTimeOffset.UtcNow
        };

        if (!string.IsNullOrWhiteSpace(requestedModel))
        {
            context.Metadata["requested.model"] = requestedModel;
        }

        return context;
    }
}
