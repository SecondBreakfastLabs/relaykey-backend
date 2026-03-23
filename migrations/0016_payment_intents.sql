ALTER TABLE payment_intents
ADD COLUMN IF NOT EXISTS verified_at timestamptz NULL;

-- Deduplicate payment_id before adding the unique index.
WITH ranked AS (
  SELECT
    id,
    payment_id,
    ROW_NUMBER() OVER (
      PARTITION BY payment_id
      ORDER BY ts DESC, id DESC
    ) AS rn
  FROM payment_intents
  WHERE payment_id IS NOT NULL
)
UPDATE payment_intents pi
SET payment_id = NULL
FROM ranked r
WHERE pi.id = r.id
  AND r.rn > 1;

-- Deduplicate payment_token before adding the unique index.
WITH ranked AS (
  SELECT
    id,
    payment_token,
    ROW_NUMBER() OVER (
      PARTITION BY payment_token
      ORDER BY ts DESC, id DESC
    ) AS rn
  FROM payment_intents
  WHERE payment_token IS NOT NULL
)
UPDATE payment_intents pi
SET payment_token = NULL
FROM ranked r
WHERE pi.id = r.id
  AND r.rn > 1;

-- Deduplicate verified request hashes before adding the unique index.
-- Keep the newest verified row for each (virtual_key_id, request_hash),
-- and downgrade older duplicates so they no longer violate the partial unique index.
WITH ranked AS (
  SELECT
    id,
    virtual_key_id,
    request_hash,
    ROW_NUMBER() OVER (
      PARTITION BY virtual_key_id, request_hash
      ORDER BY ts DESC, id DESC
    ) AS rn
  FROM payment_intents
  WHERE status = 'verified'
)
UPDATE payment_intents pi
SET status = 'failed'
FROM ranked r
WHERE pi.id = r.id
  AND r.rn > 1;

CREATE UNIQUE INDEX IF NOT EXISTS payment_intents_payment_id_unique
ON payment_intents (payment_id)
WHERE payment_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS payment_intents_payment_token_unique
ON payment_intents (payment_token)
WHERE payment_token IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS payment_intents_verified_request_hash_unique
ON payment_intents (virtual_key_id, request_hash)
WHERE status = 'verified';