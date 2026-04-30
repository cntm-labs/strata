#!/bin/bash
set -e
TIMESTAMP=$(date +%Y%m%d%H%M%S)
BACKUP_FILE="/tmp/backup-$TIMESTAMP.sql.gz"
echo "Creating backup..."
pg_dump $DATABASE_URL | gzip > $BACKUP_FILE
echo "Uploading to S3..."
aws s3 cp $BACKUP_FILE s3://$S3_BUCKET/backups/$(basename $BACKUP_FILE)
rm $BACKUP_FILE
echo "Backup complete!"
