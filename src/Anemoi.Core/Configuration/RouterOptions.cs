using System.ComponentModel.DataAnnotations;
using Anemoi.Core.Models;

namespace Anemoi.Core.Configuration;

public sealed class RouterOptions
{
    public const string SectionName = "Router";

    [Required]
    public string DefaultAlias { get; set; } = string.Empty;

    public bool EnableFallback { get; set; } = true;

    public List<BackendOptions> Backends { get; set; } = [];

    public List<ProfileOptions> Profiles { get; set; } = [];

    public List<AliasOptions> Aliases { get; set; } = [];

    public List<RuleOptions> Rules { get; set; } = [];
}

public sealed class BackendOptions
{
    [Required]
    public string Id { get; set; } = string.Empty;

    public BackendType Type { get; set; }

    [Required]
    public string BaseUrl { get; set; } = string.Empty;

    [Range(1, 600)]
    public int TimeoutSeconds { get; set; } = 120;

    public bool Enabled { get; set; } = true;

    public Dictionary<string, string> Metadata { get; set; } = new(StringComparer.OrdinalIgnoreCase);
}

public sealed class ProfileOptions
{
    [Required]
    public string ProfileId { get; set; } = string.Empty;

    [Required]
    public string BackendId { get; set; } = string.Empty;

    [Required]
    public string UpstreamModel { get; set; } = string.Empty;

    public double? Temperature { get; set; }

    public double? TopP { get; set; }

    public int? MaxTokens { get; set; }

    public int CapabilityScore { get; set; }

    public ExecutionTarget ExecutionTarget { get; set; } = ExecutionTarget.Local;
}

public sealed class AliasOptions
{
    [Required]
    public string Alias { get; set; } = string.Empty;

    [Required]
    public string ProfileId { get; set; } = string.Empty;

    public string? FallbackAlias { get; set; }

    public bool VisibleToUi { get; set; } = true;
}

public sealed class RuleOptions
{
    [Required]
    public string Name { get; set; } = string.Empty;

    [Required]
    public string Alias { get; set; } = string.Empty;

    public int Priority { get; set; }

    public List<string> MatchAnyKeywords { get; set; } = [];
}
