using Microsoft.Extensions.Options;
using Anemoi.Core.Configuration;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Core.Services;

public sealed class RouteSelector : IRouteSelector
{
    private readonly string _defaultAlias;
    private readonly IReadOnlyList<RuleOptions> _rules;
    private readonly IProfileResolver _profileResolver;

    public RouteSelector(IOptions<RouterOptions> options, IProfileResolver profileResolver)
    {
        _defaultAlias = options.Value.DefaultAlias;
        _profileResolver = profileResolver;
        _rules = options.Value.Rules
            .OrderByDescending(static rule => rule.Priority)
            .ThenBy(static rule => rule.Name, StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }

    public RouteDecision SelectRoute(RouterChatRequest request)
    {
        if (!string.IsNullOrWhiteSpace(request.RequestedModel) &&
            _profileResolver.TryResolveAlias(request.RequestedModel, out _))
        {
            return BuildDecision(request.RequestedModel!, "explicit-alias");
        }

        var normalizedContent = string.Join(
                ' ',
                request.Messages.Select(static message => message.Content).Where(static content => !string.IsNullOrWhiteSpace(content)))
            .ToLowerInvariant();

        foreach (var rule in _rules)
        {
            if (rule.MatchAnyKeywords.Any(keyword => normalizedContent.Contains(keyword, StringComparison.OrdinalIgnoreCase)))
            {
                return BuildDecision(rule.Alias, $"rule:{rule.Name}");
            }
        }

        return BuildDecision(_defaultAlias, "default-alias");
    }

    public RouteDecision? ResolveFallbackRoute(RouteDecision currentDecision)
    {
        if (string.IsNullOrWhiteSpace(currentDecision.FallbackAlias))
        {
            return null;
        }

        if (string.Equals(currentDecision.SelectedAlias, currentDecision.FallbackAlias, StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        return BuildDecision(currentDecision.FallbackAlias!, $"fallback:{currentDecision.SelectedAlias}");
    }

    private RouteDecision BuildDecision(string alias, string reason)
    {
        var aliasDefinition = _profileResolver.ResolveAlias(alias);
        var profileDefinition = _profileResolver.ResolveProfile(aliasDefinition.ProfileId);

        if (string.IsNullOrWhiteSpace(profileDefinition.BackendId))
        {
            throw new ConfigurationException($"Profile '{profileDefinition.ProfileId}' does not define a backend.");
        }

        return new RouteDecision(
            aliasDefinition.Alias,
            profileDefinition.ProfileId,
            profileDefinition.BackendId,
            profileDefinition.UpstreamModel,
            profileDefinition.Temperature,
            profileDefinition.TopP,
            profileDefinition.MaxTokens,
            reason,
            aliasDefinition.FallbackAlias);
    }
}
