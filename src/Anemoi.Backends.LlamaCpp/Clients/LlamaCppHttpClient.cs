using System.Net.Http.Headers;
using Anemoi.Core.Models;

namespace Anemoi.Backends.LlamaCpp.Clients;

public sealed class LlamaCppHttpClient
{
    private readonly HttpClient _httpClient;

    public LlamaCppHttpClient(HttpClient httpClient)
    {
        _httpClient = httpClient;
        _httpClient.DefaultRequestHeaders.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
    }

    public async Task<HttpResponseMessage> SendAsync(
        BackendDescriptor backend,
        HttpRequestMessage request,
        HttpCompletionOption completionOption,
        CancellationToken cancellationToken)
    {
        request.RequestUri = request.RequestUri is null
            ? backend.BaseUrl
            : new Uri(backend.BaseUrl, request.RequestUri);

        using var timeoutCts = new CancellationTokenSource(backend.Timeout);
        using var linkedCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken, timeoutCts.Token);

        return await _httpClient.SendAsync(request, completionOption, linkedCts.Token);
    }
}
