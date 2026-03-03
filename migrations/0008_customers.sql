-- 1. Customers table 
CREATE TABLE IF NOT EXISTS customers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), 
    name TEXT NOT NULL UNIQUE, 
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
); 

-- 2. Add customer_id to virtual keys 
ALTER TABLE virtual_keys
ADD COLUMN IF NOT EXISTS customer_id UUID REFERENCES customers(id); 

CREATE INDEX IF NOT EXISTS idx_virtual_keys_customer_id 
ON virtual_keys(customer_id); 

-- 3. Add customer_id to usage_events 
ALTER TABLE usage_events 
ADD COLUMN IF NOT EXISTS customer_id UUID REFERENCES customers(id); 

CREATE INDEX IF NOT EXISTS idx_usage_events_customer_id_ts 
ON usage_events(customer_id, ts DESC); 