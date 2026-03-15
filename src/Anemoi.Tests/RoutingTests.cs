using Microsoft.Extensions.Logging.Abstractions;
using Microsoft.Extensions.Options;
using Moq;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;
using Anemoi.Core.Services;

namespace Anemoi.Tests;

public sealed class RoutingTests
{
    private readonly IProfileResolver _profileResolver;
    private readonly IRouteSelector _routeSelector;

    public RoutingTests()
    {
        var options = Options.Create(TestConfiguration.CreateRouterOptions());
        _profileResolver = new ProfileResolver(options);
        _routeSelector = new RouteSelector(options, _profileResolver);
    }

    [Fact]
    public void SelectRoute_HonorsExplicitAlias()
    {
        var request = new RouterChatRequest("code", [ new RouterMessage("user", "hello") ], false, null, null, null);

        var decision = _routeSelector.SelectRoute(request);

        Assert.Equal("code", decision.SelectedAlias);
        Assert.Equal("code-profile", decision.SelectedProfile);
    }

    [Fact]
    public void SelectRoute_UsesKeywordRule()
    {
        var request = new RouterChatRequest(null, [ new RouterMessage("user", "Please debug this compile failure") ], false, null, null, null);

        var decision = _routeSelector.SelectRoute(request);

        Assert.Equal("code", decision.SelectedAlias);
        Assert.Equal("rule:code-keywords", decision.RoutingReason);
    }

    [Fact]
    public void SelectRoute_FallsBackToDefaultAlias()
    {
        var request = new RouterChatRequest(null, [ new RouterMessage("user", "Tell me a story") ], false, null, null, null);

        var decision = _routeSelector.SelectRoute(request);

        Assert.Equal("default-chat", decision.SelectedAlias);
    }

    [Fact]
    public void ProfileResolver_ReturnsVisibleAliases()
    {
        var aliases = _profileResolver.GetVisibleAliases();

        Assert.Contains(aliases, alias => alias.Alias == "default-chat");
        Assert.Contains(aliases, alias => alias.Alias == "code");
    }

    [Fact]
    public void BackendRegistry_ResolvesConfiguredBackend()
    {
        var backend = new Mock<IChatBackend>();
        backend.SetupGet(static b => b.Descriptor)
            .Returns(new BackendDescriptor("ollama-main", BackendType.Ollama, new Uri("http://ollama.test"), TimeSpan.FromSeconds(30), true, new Dictionary<string, string>()));

        var registry = new BackendRegistry([ backend.Object ], NullLogger<BackendRegistry>.Instance);

        var resolved = registry.GetBackend("ollama-main");

        Assert.Same(backend.Object, resolved);
    }
}
