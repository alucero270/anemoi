using Microsoft.Extensions.Options;
using Anemoi.Core.Configuration;
using Anemoi.Core.Exceptions;
using Anemoi.Core.Interfaces;
using Anemoi.Core.Models;

namespace Anemoi.Core.Services;

public sealed class ProfileResolver : IProfileResolver
{
    private readonly IReadOnlyDictionary<string, AliasDefinition> _aliases;
    private readonly IReadOnlyDictionary<string, ProfileDefinition> _profiles;

    public ProfileResolver(IOptions<RouterOptions> options)
    {
        _aliases = options.Value.Aliases
            .Select(static alias => new AliasDefinition(alias.Alias, alias.ProfileId, alias.FallbackAlias, alias.VisibleToUi))
            .ToDictionary(static alias => alias.Alias, StringComparer.OrdinalIgnoreCase);

        _profiles = options.Value.Profiles
            .Select(static profile => new ProfileDefinition(
                profile.ProfileId,
                profile.BackendId,
                profile.UpstreamModel,
                profile.Temperature,
                profile.TopP,
                profile.MaxTokens,
                profile.CapabilityScore,
                profile.ExecutionTarget))
            .ToDictionary(static profile => profile.ProfileId, StringComparer.OrdinalIgnoreCase);
    }

    public AliasDefinition ResolveAlias(string alias)
    {
        if (!_aliases.TryGetValue(alias, out var aliasDefinition))
        {
            throw new RouteNotFoundException($"Alias '{alias}' is not configured.");
        }

        return aliasDefinition;
    }

    public bool TryResolveAlias(string alias, out AliasDefinition? aliasDefinition) =>
        _aliases.TryGetValue(alias, out aliasDefinition);

    public ProfileDefinition ResolveProfile(string profileId)
    {
        if (!_profiles.TryGetValue(profileId, out var profileDefinition))
        {
            throw new ProfileResolutionException($"Profile '{profileId}' is not configured.");
        }

        return profileDefinition;
    }

    public IReadOnlyCollection<AliasDefinition> GetVisibleAliases() =>
        _aliases.Values.Where(static alias => alias.VisibleToUi).OrderBy(static alias => alias.Alias, StringComparer.OrdinalIgnoreCase).ToArray();
}
