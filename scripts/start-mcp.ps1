#!/usr/bin/pwsh
# Starts anemoi-mcp for Pi integration
# This script can be called from Pi to launch the MCP adapter

$ErrorActionPreference = "Stop"

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$ROOT_DIR = Split-Path -Parent $SCRIPT_DIR

# Default port for MCP adapter
$PORT = $env:ANEMOI_MCP_PORT ?? 7072

Write-Host "Starting Anemoi MCP adapter on port $PORT..."
Write-Host "Press Ctrl+C to stop"

$cargoPath = "cargo"
$command = "$cargoPath run -p anemoi-mcp -- serve --port $PORT"

# Run the command and pass through stdout/stderr
& $command
exit $LASTEXITCODE
