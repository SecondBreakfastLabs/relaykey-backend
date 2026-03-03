-- 1) ensure default customer exists
INSERT INTO customers (name)
VALUES ('default')
ON CONFLICT (name) DO NOTHING;

-- 2) backfill virtual_keys.customer_id
UPDATE virtual_keys vk
SET customer_id = c.id
FROM customers c
WHERE c.name = 'default'
  AND vk.customer_id IS NULL;

-- 3) enforce not-null
ALTER TABLE virtual_keys
ALTER COLUMN customer_id SET NOT NULL;