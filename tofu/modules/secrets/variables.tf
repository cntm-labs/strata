variable "name_prefix" {
  description = "Prefix for secret names (e.g. \"strata-prod\")"
  type        = string
}

variable "nucleus_secret_key" {
  description = "Nucleus JWT signing key. Empty string disables auth."
  type        = string
  sensitive   = true
  default     = ""
}

variable "resend_api_key" {
  description = "Resend API key. Empty string disables email."
  type        = string
  sensitive   = true
  default     = ""
}

variable "sentry_dsn" {
  description = "Sentry DSN. Empty string disables error capture."
  type        = string
  sensitive   = true
  default     = ""
}
