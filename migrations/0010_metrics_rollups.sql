-- Phase 8: Metrics rollups 

CREATE TABLE IF NOT EXISTS usage_rollup_daily (
    day date NOT NULL, 
    customer_id uuid NOT NULL, 
    virtual_key_id uuid NOT NULL, 
    partner_name text NOT NULL, 

    total_requests bigint NOT NULL, 
    forwarded_requests bigint NOT NULL, 
    blocked_requests bigint NOT NULL, 

    avg_latency_ms double precision NOT NULL, 

    status_2xx bigint NOT NULL, 
    status_3xx bigint NOT NULL, 
    status_4xx bigint NOT NULL, 
    status_5xx bigint NOT NULL, 

    PRIMARY KEY (day, customer_id, virtual_key_id, partner_name)
); 

CREATE TABLE IF NOT EXISTS error_rollup_daily (
    day date NOT NULL, 
    customer_id uuid NOT NULL, 
    virtual_key_id uuid NOT NULL, 
    partner_name text NOT NULL, 

    error_bucket text NOT NULL, 
    count bigint NOT NULL, 

    PRIMARY KEY (day, customer_id, virtual_key_id, partner_name, error_bucket)
); 

CREATE INDEX IF NOT EXISTS idx_usage_rollup_daily_day
ON usage_rollup_daily(day); 

CREATE INDEX IF NOT EXISTS idx_error_rollup_daily_day 
ON error_rollup_daily(day); 

CREATE INDEX IF NOT EXISTS usage_events_ts_idx 
ON usage_events(ts DESC);