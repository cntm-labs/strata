variable "name" {
  description = "Name prefix applied to all resources"
  type        = string
}

variable "cidr_block" {
  description = "VPC CIDR block"
  type        = string
  default     = "10.0.0.0/16"
}

variable "azs" {
  description = "Availability zones to use. Empty list = pick first 2 in region."
  type        = list(string)
  default     = []
}

variable "single_nat_gateway" {
  description = "If true, use one NAT Gateway shared by all private subnets (cost saver, no HA)."
  type        = bool
  default     = false
}
