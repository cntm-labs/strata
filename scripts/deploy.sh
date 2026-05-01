#!/bin/bash
set -euo pipefail

ENV_FILE="${ENV_FILE:-.env.production}"

if [[ ! -f "$ENV_FILE" ]]; then
    echo "Error: $ENV_FILE not found." >&2
    echo "Copy .env.production.example to $ENV_FILE and fill in real values." >&2
    exit 1
fi

# Load env file into the script's environment so we can validate values.
set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

required=(STRATA_DOMAIN ACME_EMAIL POSTGRES_PASSWORD STRATA_APP_PASSWORD)
for var in "${required[@]}"; do
    value="${!var:-}"
    if [[ -z "$value" || "$value" == *"__CHANGE_ME__"* ]]; then
        echo "Error: $var is unset or still has the __CHANGE_ME__ placeholder." >&2
        echo "Edit $ENV_FILE and set a real value for $var." >&2
        exit 1
    fi
done

echo "Bringing up production stack…"
docker compose \
    -f docker-compose.yml \
    -f docker-compose.production.yml \
    --env-file "$ENV_FILE" \
    up -d --build

echo "Waiting up to 60s for strata to report healthy…"
deadline=$(( $(date +%s) + 60 ))
state=""
while [[ $(date +%s) -lt $deadline ]]; do
    state=$(docker compose \
        -f docker-compose.yml \
        -f docker-compose.production.yml \
        --env-file "$ENV_FILE" \
        ps --format json strata 2>/dev/null \
        | grep -o '"Health":"[^"]*"' | head -1 | cut -d'"' -f4 || true)
    if [[ "$state" == "healthy" ]]; then
        break
    fi
    sleep 2
done

if [[ "$state" != "healthy" ]]; then
    echo "Warning: strata did not become healthy within 60s. Recent logs:" >&2
    docker compose \
        -f docker-compose.yml \
        -f docker-compose.production.yml \
        --env-file "$ENV_FILE" \
        logs --tail 30 strata >&2
    exit 1
fi

cat <<EOF

✓ Strata is live at https://${STRATA_DOMAIN}

Tail logs with:
  docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file ${ENV_FILE} logs -f strata

Tail Caddy (TLS / ACME) with:
  docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file ${ENV_FILE} logs -f caddy
EOF
