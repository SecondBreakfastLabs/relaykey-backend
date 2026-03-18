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