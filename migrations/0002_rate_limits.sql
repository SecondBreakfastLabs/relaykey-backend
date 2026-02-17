ALTER TABLE virtual_keys
ADD COLUMN IF NOT EXISTS rps_limit integer NULL,
ADD COLUMN IF NOT EXISTS rps_burst integer NULL,
ADD COLUMN IF NOT EXISTS monthly_quota integer NULL;
