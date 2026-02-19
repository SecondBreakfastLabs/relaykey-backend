-- 1) add column nullable first
ALTER TABLE virtual_keys
ADD COLUMN IF NOT EXISTS policy_id uuid;

-- 2) create a default policy if you want to seed it here
-- (optional: if you prefer manual SQL, skip this block)
INSERT INTO policies (name, endpoint_allowlist, rps_limit, rps_burst, monthly_quota, timeout_ms)
VALUES ('dev-default', ARRAY['/v1/*'], 5, 5, 1000, 30000)
ON CONFLICT (name) DO NOTHING;

-- 3) backfill existing keys to that default policy
UPDATE virtual_keys
SET policy_id = (SELECT id FROM policies WHERE name = 'dev-default' LIMIT 1)
WHERE policy_id IS NULL;

-- 4) now enforce NOT NULL + FK
ALTER TABLE virtual_keys
ALTER COLUMN policy_id SET NOT NULL;

ALTER TABLE virtual_keys
ADD CONSTRAINT virtual_keys_policy_id_fkey
FOREIGN KEY (policy_id) REFERENCES policies(id);
