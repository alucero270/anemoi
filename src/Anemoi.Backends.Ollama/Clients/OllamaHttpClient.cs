using System.Net.Http.Headers;
using System.Net.Security;
using System.Security.Cryptography.X509Certificates;
using System.Collections.Concurrent;
using Anemoi.Core.Models;

namespace Anemoi.Backends.Ollama.Clients;

public sealed class OllamaHttpClient
{
    private readonly HttpClient _httpClient;
    private readonly ConcurrentDictionary<string, HttpClient> _insecureClients = new(StringComparer.OrdinalIgnoreCase);

    public OllamaHttpClient(HttpClient httpClient)
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

        return await GetHttpClient(backend).SendAsync(request, completionOption, linkedCts.Token);
    }

    private HttpClient GetHttpClient(BackendDescriptor backend)
    {
        if (!backend.AllowInsecureTls)
        {
            return _httpClient;
        }

        return _insecureClients.GetOrAdd(backend.BaseUrl.Authority, _ =>
        {
            var handler = new HttpClientHandler
            {
                ServerCertificateCustomValidationCallback = static (_, _, _, _) => true
            };

            var insecureClient = new HttpClient(handler, disposeHandler: true);
            insecureClient.DefaultRequestHeaders.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
            return insecureClient;
        });
    }
}
