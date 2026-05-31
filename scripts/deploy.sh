#!/bin/bash
# scripts/deploy.sh
set -e

# Check if .env.production exists
if [ ! -f .env.production ]; then
    echo "Error: .env.production not found. Please create it from .env.production.example"
    exit 1
fi

# Simple validation for required variables
REQUIRED_VARS=("DATABASE_URL" "NUCLEUS_API_KEY" "STRATA_DOMAIN" "ACME_EMAIL")
for var in "${REQUIRED_VARS[@]}"; do
    if ! grep -q "^$var=" .env.production; then
        echo "Warning: Required variable $var might be missing in .env.production"
    fi
done

echo "Building and starting Strata Production Stack..."
docker compose -f docker-compose.production.yml up -d --build

echo "Deployment initiated successfully!"
echo "Dashboard: https://dashboard.<your-domain>"
echo "API: https://api.<your-domain>"
