using Microsoft.AspNetCore.Mvc;
using Anemoi.Api.Models;
using Anemoi.Core.Interfaces;

namespace Anemoi.Api.Controllers;

[ApiController]
[Route("v1/models")]
public sealed class ModelsController : ControllerBase
{
    private readonly IProfileResolver _profileResolver;

    public ModelsController(IProfileResolver profileResolver)
    {
        _profileResolver = profileResolver;
    }

    [HttpGet]
    public ActionResult<ModelListResponseDto> Get()
    {
        var created = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var models = _profileResolver.GetVisibleAliases()
            .Select(alias => new ModelDto
            {
                Id = alias.Alias,
                Created = created
            })
            .ToArray();

        return Ok(new ModelListResponseDto { Data = models });
    }
}
