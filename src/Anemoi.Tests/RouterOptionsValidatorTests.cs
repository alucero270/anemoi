using Microsoft.Extensions.Options;
using Anemoi.Core.Services;

namespace Anemoi.Tests;

public sealed class RouterOptionsValidatorTests
{
    [Fact]
    public void Validate_Succeeds_ForValidConfiguration()
    {
        var validator = new RouterOptionsValidator();

        var result = validator.Validate(Options.DefaultName, TestConfiguration.CreateRouterOptions());

        Assert.True(result.Succeeded);
    }

    [Fact]
    public void Validate_Fails_WhenDefaultAliasIsMissing()
    {
        var options = TestConfiguration.CreateRouterOptions();
        options.DefaultAlias = "missing";
        var validator = new RouterOptionsValidator();

        var result = validator.Validate(Options.DefaultName, options);

        Assert.False(result.Succeeded);
        Assert.Contains(result.Failures!, failure => failure.Contains("DefaultAlias", StringComparison.OrdinalIgnoreCase));
    }
}
