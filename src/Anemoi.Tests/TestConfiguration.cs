using Anemoi.Core.Configuration;

namespace Anemoi.Tests;

internal static class TestConfiguration
{
    public static RouterOptions CreateRouterOptions() =>
        new()
        {
            DefaultAlias = "default-chat",
            EnableFallback = true,
            Backends =
            [
                new BackendOptions
                {
                    Id = "ollama-main",
                    Type = Anemoi.Core.Models.BackendType.Ollama,
                    BaseUrl = "http://ollama.test",
                    TimeoutSeconds = 30,
                    Enabled = true
                },
                new BackendOptions
                {
                    Id = "llamacpp-main",
                    Type = Anemoi.Core.Models.BackendType.LlamaCpp,
                    BaseUrl = "http://llamacpp.test",
                    TimeoutSeconds = 30,
                    Enabled = true
                }
            ],
            Profiles =
            [
                new ProfileOptions
                {
                    ProfileId = "default-chat-profile",
                    BackendId = "ollama-main",
                    UpstreamModel = "llama3.1:8b",
                    Temperature = 0.7,
                    TopP = 0.95,
                    MaxTokens = 512,
                    ExecutionTarget = Anemoi.Core.Models.ExecutionTarget.Local
                },
                new ProfileOptions
                {
                    ProfileId = "code-profile",
                    BackendId = "llamacpp-main",
                    UpstreamModel = "qwen2.5-coder",
                    Temperature = 0.2,
                    TopP = 0.9,
                    MaxTokens = 1024,
                    ExecutionTarget = Anemoi.Core.Models.ExecutionTarget.Local
                },
                new ProfileOptions
                {
                    ProfileId = "fast-profile",
                    BackendId = "llamacpp-main",
                    UpstreamModel = "phi-3-mini",
                    Temperature = 0.6,
                    TopP = 0.9,
                    MaxTokens = 256,
                    ExecutionTarget = Anemoi.Core.Models.ExecutionTarget.Local
                }
            ],
            Aliases =
            [
                new AliasOptions
                {
                    Alias = "default-chat",
                    ProfileId = "default-chat-profile",
                    FallbackAlias = "fast",
                    VisibleToUi = true
                },
                new AliasOptions
                {
                    Alias = "code",
                    ProfileId = "code-profile",
                    FallbackAlias = "default-chat",
                    VisibleToUi = true
                },
                new AliasOptions
                {
                    Alias = "fast",
                    ProfileId = "fast-profile",
                    FallbackAlias = "default-chat",
                    VisibleToUi = true
                }
            ],
            Rules =
            [
                new RuleOptions
                {
                    Name = "code-keywords",
                    Alias = "code",
                    Priority = 100,
                    MatchAnyKeywords = [ "compile", "debug", "stack trace" ]
                }
            ]
        };

    public static Dictionary<string, string?> ToDictionary(RouterOptions options)
    {
        var values = new Dictionary<string, string?>(StringComparer.OrdinalIgnoreCase)
        {
            ["Router:DefaultAlias"] = options.DefaultAlias,
            ["Router:EnableFallback"] = options.EnableFallback.ToString()
        };

        for (var i = 0; i < options.Backends.Count; i++)
        {
            var backend = options.Backends[i];
            values[$"Router:Backends:{i}:Id"] = backend.Id;
            values[$"Router:Backends:{i}:Type"] = backend.Type.ToString();
            values[$"Router:Backends:{i}:BaseUrl"] = backend.BaseUrl;
            values[$"Router:Backends:{i}:TimeoutSeconds"] = backend.TimeoutSeconds.ToString();
            values[$"Router:Backends:{i}:Enabled"] = backend.Enabled.ToString();
            values[$"Router:Backends:{i}:AllowInsecureTls"] = backend.AllowInsecureTls.ToString();
        }

        for (var i = 0; i < options.Profiles.Count; i++)
        {
            var profile = options.Profiles[i];
            values[$"Router:Profiles:{i}:ProfileId"] = profile.ProfileId;
            values[$"Router:Profiles:{i}:BackendId"] = profile.BackendId;
            values[$"Router:Profiles:{i}:UpstreamModel"] = profile.UpstreamModel;
            values[$"Router:Profiles:{i}:Temperature"] = profile.Temperature?.ToString(System.Globalization.CultureInfo.InvariantCulture);
            values[$"Router:Profiles:{i}:TopP"] = profile.TopP?.ToString(System.Globalization.CultureInfo.InvariantCulture);
            values[$"Router:Profiles:{i}:MaxTokens"] = profile.MaxTokens?.ToString();
            values[$"Router:Profiles:{i}:CapabilityScore"] = profile.CapabilityScore.ToString();
            values[$"Router:Profiles:{i}:ExecutionTarget"] = profile.ExecutionTarget.ToString();
        }

        for (var i = 0; i < options.Aliases.Count; i++)
        {
            var alias = options.Aliases[i];
            values[$"Router:Aliases:{i}:Alias"] = alias.Alias;
            values[$"Router:Aliases:{i}:ProfileId"] = alias.ProfileId;
            values[$"Router:Aliases:{i}:FallbackAlias"] = alias.FallbackAlias;
            values[$"Router:Aliases:{i}:VisibleToUi"] = alias.VisibleToUi.ToString();
        }

        for (var i = 0; i < options.Rules.Count; i++)
        {
            var rule = options.Rules[i];
            values[$"Router:Rules:{i}:Name"] = rule.Name;
            values[$"Router:Rules:{i}:Alias"] = rule.Alias;
            values[$"Router:Rules:{i}:Priority"] = rule.Priority.ToString();

            for (var j = 0; j < rule.MatchAnyKeywords.Count; j++)
            {
                values[$"Router:Rules:{i}:MatchAnyKeywords:{j}"] = rule.MatchAnyKeywords[j];
            }
        }

        return values;
    }
}
