-- x402 metrics maturity

CREATE TABLE IF NOT EXISTS x402_rollup_daily (
  day date NOT NULL,
  customer_id uuid NOT NULL,
  virtual_key_id uuid NOT NULL,
  partner_name text NOT NULL,
  provider text NOT NULL,

  intents_created bigint NOT NULL,
  verified_count bigint NOT NULL,
  failed_count bigint NOT NULL,
  expired_count bigint NOT NULL,
  unpaid_count bigint NOT NULL,

  revenue_cents bigint NOT NULL DEFAULT 0,

  PRIMARY KEY (day, customer_id, virtual_key_id, partner_name, provider)
);

CREATE INDEX IF NOT EXISTS idx_x402_rollup_daily_day
ON x402_rollup_daily(day);

CREATE TABLE IF NOT EXISTS x402_events (
  id bigserial PRIMARY KEY,
  ts timestamptz NOT NULL DEFAULT now(),

  customer_id uuid NOT NULL,
  virtual_key_id uuid NOT NULL REFERENCES virtual_keys(id) ON DELETE CASCADE,
  partner_name text NOT NULL,
  provider text NOT NULL,
  path text NOT NULL,

  event_type text NOT NULL,
  detail text NULL
);

CREATE INDEX IF NOT EXISTS x402_events_vk_ts_idx
ON x402_events (virtual_key_id, ts DESC);

CREATE INDEX IF NOT EXISTS x402_events_customer_ts_idx
ON x402_events (customer_id, ts DESC);

CREATE TABLE IF NOT EXISTS x402_error_rollup_daily (
  day date NOT NULL,
  customer_id uuid NOT NULL,
  virtual_key_id uuid NOT NULL,
  partner_name text NOT NULL,
  provider text NOT NULL,
  error_bucket text NOT NULL,
  count bigint NOT NULL,

  PRIMARY KEY (day, customer_id, virtual_key_id, partner_name, provider, error_bucket)
);

CREATE INDEX IF NOT EXISTS idx_x402_error_rollup_daily_day
ON x402_error_rollup_daily(day);