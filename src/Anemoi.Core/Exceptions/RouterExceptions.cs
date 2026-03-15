namespace Anemoi.Core.Exceptions;

public class RouterException : Exception
{
    public RouterException(string message)
        : base(message)
    {
    }

    public RouterException(string message, Exception innerException)
        : base(message, innerException)
    {
    }
}

public sealed class RouteNotFoundException : RouterException
{
    public RouteNotFoundException(string message)
        : base(message)
    {
    }
}

public sealed class ProfileResolutionException : RouterException
{
    public ProfileResolutionException(string message)
        : base(message)
    {
    }
}

public sealed class BackendUnavailableException : RouterException
{
    public BackendUnavailableException(string message)
        : base(message)
    {
    }

    public BackendUnavailableException(string message, Exception innerException)
        : base(message, innerException)
    {
    }
}

public sealed class UpstreamProtocolException : RouterException
{
    public UpstreamProtocolException(string message)
        : base(message)
    {
    }

    public UpstreamProtocolException(string message, Exception innerException)
        : base(message, innerException)
    {
    }
}

public sealed class ConfigurationException : RouterException
{
    public ConfigurationException(string message)
        : base(message)
    {
    }
}
