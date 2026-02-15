CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS partners (
  id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
  name text NOT NULL UNIQUE,
  base_url text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS upstream_credentials (
  id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
  partner_id uuid NOT NULL REFERENCES partners(id) ON DELETE CASCADE,
  header_name text NOT NULL,
  header_value text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (partner_id, header_name)
);

CREATE TABLE IF NOT EXISTS virtual_keys (
  id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
  name text NOT NULL,
  key_hash text NOT NULL UNIQUE,
  enabled boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz NULL
);
