using Microsoft.Extensions.Options;
using Anemoi.Core.Configuration;
using Anemoi.Core.Models;

namespace Anemoi.Core.Services;

public sealed class RouterOptionsValidator : IValidateOptions<RouterOptions>
{
    public ValidateOptionsResult Validate(string? name, RouterOptions options)
    {
        var errors = new List<string>();

        if (string.IsNullOrWhiteSpace(options.DefaultAlias))
        {
            errors.Add("Router:DefaultAlias is required.");
        }

        if (options.Backends.Count == 0)
        {
            errors.Add("Router:Backends must contain at least one backend.");
        }

        if (options.Profiles.Count == 0)
        {
            errors.Add("Router:Profiles must contain at least one profile.");
        }

        if (options.Aliases.Count == 0)
        {
            errors.Add("Router:Aliases must contain at least one alias.");
        }

        errors.AddRange(ValidateUniqueness(options.Backends.Select(static backend => backend.Id), "backend"));
        errors.AddRange(ValidateUniqueness(options.Profiles.Select(static profile => profile.ProfileId), "profile"));
        errors.AddRange(ValidateUniqueness(options.Aliases.Select(static alias => alias.Alias), "alias"));
        errors.AddRange(ValidateUniqueness(options.Rules.Select(static rule => rule.Name), "rule"));

        var backendIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var backend in options.Backends)
        {
            if (string.IsNullOrWhiteSpace(backend.Id))
            {
                errors.Add("Each backend requires a non-empty Id.");
                continue;
            }

            backendIds.Add(backend.Id);

            if (!Uri.TryCreate(backend.BaseUrl, UriKind.Absolute, out _))
            {
                errors.Add($"Backend '{backend.Id}' has an invalid BaseUrl.");
            }

            if (backend.TimeoutSeconds <= 0)
            {
                errors.Add($"Backend '{backend.Id}' must have a positive TimeoutSeconds value.");
            }

            if (backend.Type is BackendType.FoundryLocal && backend.Enabled)
            {
                errors.Add($"Backend '{backend.Id}' uses FoundryLocal, which is reserved and not supported in Stage 1.");
            }
        }

        if (!options.Backends.Any(static backend => backend.Enabled))
        {
            errors.Add("At least one backend must be enabled.");
        }

        var profileIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var profile in options.Profiles)
        {
            if (string.IsNullOrWhiteSpace(profile.ProfileId))
            {
                errors.Add("Each profile requires a non-empty ProfileId.");
                continue;
            }

            profileIds.Add(profile.ProfileId);

            if (!backendIds.Contains(profile.BackendId))
            {
                errors.Add($"Profile '{profile.ProfileId}' references unknown backend '{profile.BackendId}'.");
            }

            if (string.IsNullOrWhiteSpace(profile.UpstreamModel))
            {
                errors.Add($"Profile '{profile.ProfileId}' requires an UpstreamModel.");
            }
        }

        var aliasIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var alias in options.Aliases)
        {
            if (string.IsNullOrWhiteSpace(alias.Alias))
            {
                errors.Add("Each alias requires a non-empty Alias value.");
                continue;
            }

            aliasIds.Add(alias.Alias);

            if (!profileIds.Contains(alias.ProfileId))
            {
                errors.Add($"Alias '{alias.Alias}' references unknown profile '{alias.ProfileId}'.");
            }
        }

        if (!aliasIds.Contains(options.DefaultAlias))
        {
            errors.Add($"Router:DefaultAlias '{options.DefaultAlias}' does not match a configured alias.");
        }

        foreach (var alias in options.Aliases.Where(static alias => !string.IsNullOrWhiteSpace(alias.FallbackAlias)))
        {
            if (!aliasIds.Contains(alias.FallbackAlias!))
            {
                errors.Add($"Alias '{alias.Alias}' references unknown fallback alias '{alias.FallbackAlias}'.");
            }
        }

        foreach (var rule in options.Rules)
        {
            if (!aliasIds.Contains(rule.Alias))
            {
                errors.Add($"Rule '{rule.Name}' references unknown alias '{rule.Alias}'.");
            }

            if (rule.MatchAnyKeywords.Count == 0)
            {
                errors.Add($"Rule '{rule.Name}' must declare at least one MatchAnyKeywords value.");
            }
        }

        return errors.Count > 0
            ? ValidateOptionsResult.Fail(errors)
            : ValidateOptionsResult.Success;
    }

    private static IEnumerable<string> ValidateUniqueness(IEnumerable<string> values, string label)
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (var value in values.Where(static value => !string.IsNullOrWhiteSpace(value)))
        {
            if (!seen.Add(value))
            {
                yield return $"Duplicate {label} identifier '{value}' was found.";
            }
        }
    }
}
