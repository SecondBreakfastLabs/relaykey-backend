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