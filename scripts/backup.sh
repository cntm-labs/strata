#!/bin/bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL must be set}"
: "${BACKUP_INTERVAL:=86400}"
: "${RETENTION_DAYS:=14}"
BACKUP_DIR="${BACKUP_DIR:-/backups}"

mkdir -p "$BACKUP_DIR"

# TODO(prod): swap superuser DATABASE_URL for a dedicated `strata_backup`
# role with pg_read_all_data once the admin tooling for role provisioning lands.

while true; do
    ts=$(date -u +%Y%m%dT%H%M%SZ)
    file="${BACKUP_DIR}/strata-${ts}.sql.gz"
    echo "[$(date -u -Iseconds)] Starting backup → $file"

    pg_dump "$DATABASE_URL" | gzip > "$file"
    echo "[$(date -u -Iseconds)] Backup complete: $(du -h "$file" | cut -f1)"

    if [[ -n "${S3_BUCKET:-}" ]]; then
        aws s3 cp "$file" "s3://${S3_BUCKET}/backups/$(basename "$file")"
        echo "[$(date -u -Iseconds)] Uploaded to s3://${S3_BUCKET}/"
    fi

    # Prune old local backups (S3 retention managed via lifecycle rules).
    find "$BACKUP_DIR" -name 'strata-*.sql.gz' -mtime +"${RETENTION_DAYS}" -delete

    sleep "${BACKUP_INTERVAL}"
done
