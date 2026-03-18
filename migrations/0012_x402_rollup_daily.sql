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

  -- later
  revenue_cents bigint NOT NULL DEFAULT 0,

  PRIMARY KEY (day, customer_id, virtual_key_id, partner_name, provider)
);

CREATE INDEX IF NOT EXISTS idx_x402_rollup_daily_day
ON x402_rollup_daily(day);