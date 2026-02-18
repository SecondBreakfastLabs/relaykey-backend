-- 1. Add columns
ALTER TABLE virtual_keys
    ADD COLUMN IF NOT EXISTS name TEXT NOT NULL DEFAULT '', 
    ADD COLUMN IF NOT EXISTS environment TEXT NOT NULL DEFAULT 'dev', 
    ADD COLUMN IF NOT EXISTS tages TEXT[] NOT NULL DEFAULT '{}'; 

-- 2. Helpful indices 
CREATE INDEX IF NOT EXISTS idx_virtual_keys_environment ON virtual_keys(environment);
CREATE INDEX IF NOT EXISTS idx_virtual_keys_tags_gin ON virtual_keys USING GIN(tags); 
CREATE INDEX IF NOT EXISTS idx_virtual_keys_enabled ON virtual_keys(enabled); 