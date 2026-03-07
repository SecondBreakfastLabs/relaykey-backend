CREATE TABLE IF NOT EXISTS payment_intents (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  ts timestamptz NOT NULL DEFAULT now(),

  virtual_key_id uuid NOT NULL REFERENCES virtual_keys(id) ON DELETE CASCADE,

  partner_name text NOT NULL,
  path text NOT NULL,

  request_hash text NOT NULL,
  amount text NOT NULL,
  currency text NOT NULL,

  facilitator_url text NOT NULL,
  recipient text NOT NULL,
  memo text NULL,
  expires_at timestamptz NULL,

  -- provider bookkeeping (keep it generic)
  provider text NOT NULL,          -- 'noop' | 'stub' | later 'xpay' etc
  status text NOT NULL,            -- 'pending' | 'verified' | 'expired' | 'failed'
  payment_id text NULL,            -- X-Payment-ID
  payment_token text NULL          -- X-Payment-Token (if you choose to store; optional)
);

CREATE INDEX IF NOT EXISTS payment_intents_vk_ts_idx
  ON payment_intents (virtual_key_id, ts DESC);

CREATE INDEX IF NOT EXISTS payment_intents_request_hash_idx
  ON payment_intents (request_hash);