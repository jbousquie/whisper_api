#!/usr/bin/env pwsh
# Generate configuration documentation and sample config file

Write-Host "Generating configuration documentation..." -ForegroundColor Green

# Generate sample config file
cargo run --bin whisper_api -- --generate-config > "whisper_api_sample.conf"
if ($LASTEXITCODE -eq 0) {
    Write-Host "Sample configuration file generated: whisper_api_sample.conf" -ForegroundColor Green
} else {
    Write-Host "Failed to generate sample configuration file" -ForegroundColor Red
}

# Generate markdown documentation
cargo run --bin whisper_api -- --generate-docs > "CONFIG.md"
if ($LASTEXITCODE -eq 0) {
    Write-Host "Configuration documentation generated: CONFIG.md" -ForegroundColor Green
} else {
    Write-Host "Failed to generate configuration documentation" -ForegroundColor Red
}

Write-Host "Documentation generation complete!" -ForegroundColor Green
