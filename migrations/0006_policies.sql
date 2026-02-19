CREATE TABLE policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), 
    name TEXT NOT NULL UNIQUE, 

    endpoint_allowlist TEXT[] NOT NULL DEFAULT '{}', 

    rps_limit INTEGER, 
    rps_burst INTEGER, 
    monthly_quota INTEGER,

    timeout_ms INTEGER NOT NULL DEFAULT 30000, 
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
); 