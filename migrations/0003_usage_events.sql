CREATE TABLE IF NOT EXISTS usage_events (
  id bigserial PRIMARY KEY,
  ts timestamptz NOT NULL DEFAULT now(),
  virtual_key_id uuid NOT NULL REFERENCES virtual_keys(id) ON DELETE CASCADE,

  partner_name text NOT NULL,
  path text NOT NULL,

  forwarded boolean NOT NULL,
  blocked_reason text NULL,      -- 'rate_limit_exceeded' | 'monthly_quota_exceeded' | null
  status_code integer NULL,
  latency_ms integer NOT NULL
);

CREATE INDEX IF NOT EXISTS usage_events_vk_ts_idx
ON usage_events (virtual_key_id, ts DESC);
