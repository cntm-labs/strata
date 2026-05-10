# Strata production deployment — OpenTofu runbook

This directory deploys Strata to AWS Fargate via the modules under
`../modules/`. The code is reviewed and tested with `tofu validate` in
CI; what's left is the operator running it against an account they own.

## Cost

Approximate monthly cost in `us-east-1` for the default settings (HA):

| Component | Cost |
|---|---|
| RDS db.t4g.small Multi-AZ | ~$30 |
| ALB | ~$20 |
| 2× NAT Gateway (one per AZ) | ~$90 |
| 2× Fargate task (0.5 vCPU / 1 GiB) | ~$30 |
| CloudWatch + Secrets Manager | ~$5 |
| **Total** | **~$175/mo** |

Cost saving toggles:
- `single_nat_gateway = true` → −$45 (loses NAT redundancy)
- `multi_az = false` → −$15 (loses RDS failover)
- `desired_count = 1` → −$15 (loses task redundancy)

Lower bound for "running but not HA": ~$100/mo.

## Prerequisites

1. **AWS account** with admin or sufficient narrower IAM (VPC, RDS, ECS, IAM, Secrets Manager, ALB, CloudWatch, SNS).
2. **OpenTofu CLI** ≥ 1.8 — install via [opentofu.org](https://opentofu.org/docs/intro/install/).
3. **A registered domain** you control DNS for. The modules don't manage Route 53.
4. **An ACM certificate** provisioned in the same region as `var.region`, covering the domain you'll point at the ALB. The ARN goes into `var.acm_certificate_arn`.
5. **An S3 bucket** + **DynamoDB table** for Terraform state (see "State backend" below).

## State backend

By default this stack uses local state (`terraform.tfstate` in this directory). For any real apply, use S3 + DynamoDB:

```bash
# 1. Create the bucket (replace YOURS):
aws s3api create-bucket --bucket YOUR-tofu-state --region us-east-1
aws s3api put-bucket-versioning --bucket YOUR-tofu-state \
    --versioning-configuration Status=Enabled
aws s3api put-bucket-encryption --bucket YOUR-tofu-state \
    --server-side-encryption-configuration '{"Rules":[{"ApplyServerSideEncryptionByDefault":{"SSEAlgorithm":"AES256"}}]}'

# 2. Create the lock table:
aws dynamodb create-table --table-name YOUR-tofu-state-locks \
    --attribute-definitions AttributeName=LockID,AttributeType=S \
    --key-schema AttributeName=LockID,KeyType=HASH \
    --billing-mode PAY_PER_REQUEST --region us-east-1
```

Then uncomment the `backend "s3"` block in `main.tf` and fill in:
- `bucket = "YOUR-tofu-state"`
- `dynamodb_table = "YOUR-tofu-state-locks"`

## First apply

```bash
# 1. Sensitive vars — never commit these to the repo.
export TF_VAR_nucleus_api_key="your-nucleus-key"
export TF_VAR_resend_api_key="re_..."
export TF_VAR_sentry_dsn="https://...@sentry.io/..."

# 2. Required vars in terraform.tfvars (NOT sensitive — safe to commit
# IF you're using a private repo; consider keeping local for safety):
cat > terraform.tfvars <<EOF
acm_certificate_arn = "arn:aws:acm:us-east-1:123456789012:certificate/abc-..."
alarm_email         = "ops@example.com"
EOF

# 3. Initialise.
tofu init

# 4. Plan — read the output carefully. ~80 resources will be created.
tofu plan

# 5. Apply.
tofu apply
```

The first apply takes ~15 minutes (RDS provisioning is the slow step). Once it returns successfully:

```bash
# Get the ALB DNS name.
tofu output alb_dns_name

# Point your domain at it. For Route 53:
# Create an A record (alias) at strata.your-domain.com → alb_dns_name (zone alias).
# For other DNS providers: a CNAME at strata.your-domain.com → <alb_dns_name>.
```

## Verification

```bash
# Once DNS resolves (TTL-bound):
curl -i https://strata.your-domain.com/api/v1/health
# → HTTP/2 200, body: {"status":"ok"}
```

If you see 502 / 503 from the ALB, the tasks are still warming up — health-check grace period is 60 seconds and the tasks need to pass `/api/v1/health` twice (30 s interval) before the ALB routes to them. Wait ~3 minutes after `tofu apply` completes.

The CloudWatch dashboard URL is in `tofu output dashboard_url` — visit it to see request rate, 5xx count, ECS task count, and recent ERROR logs.

## Common gotchas

- **ACM cert wrong region.** The ALB and the cert must be in the same region. ACM in `us-east-1` does NOT work with an ALB in `eu-west-1`. The plan fails with "certificate ARN is not valid".
- **`desired_count = 1` + `multi_az = true`** is a wasteful combo — you have RDS HA but a single task SPOF. Either bump count to 2 or drop multi_az.
- **GHCR private image.** Public `ghcr.io/cntm-labs/strata` pulls without auth. If the image goes private, the task definition needs a `repositoryCredentials` block referencing a Secrets Manager entry holding a Docker config JSON. Not in this PR; flagged as a follow-up.
- **Secrets in state.** Terraform state stores secret values in plaintext. Encrypt the state bucket (Step 2 above does this). Restrict bucket access to ops users only.
- **Database migrations.** When the strata-server task starts, it runs `sqlx::migrate!` against the admin role. The first deploy applies migrations 001–006 against a fresh database; subsequent deploys are no-ops unless a new migration was added. There is no separate "migration" step — task start IS the migration.
- **NAT egress for backups.** This Tier-3 stack does not include the backup sidecar from Tier 1; production backups rely on RDS automated snapshots (7-day retention configured in the rds module). If you want logical pg_dump backups too, add a separate ECS task or run them as a Lambda / EventBridge schedule — out of scope for this stack.

## Updating the deployed image

Bump `var.image_tag` and re-apply:

```bash
tofu apply -var image_tag=v1.2.3
```

This updates the ECS task definition; the service does a rolling deploy (default 100% min healthy / 200% max during deploy, takes ~2 minutes for 2 tasks).

## Teardown

```bash
# 1. RDS deletion protection is on by default. Disable first:
aws rds modify-db-instance --db-instance-identifier strata-prod-postgres \
    --no-deletion-protection --apply-immediately

# 2. Then destroy:
tofu destroy
```

A final RDS snapshot named `strata-prod-final` is retained — keep it or delete via Console after confirming you don't need a restore.
