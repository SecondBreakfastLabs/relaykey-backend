-- Partner
INSERT INTO partners (name, base_url)
VALUES ('example', 'https://httpbin.org')
ON CONFLICT (name) DO NOTHING;

-- Credential (example; httpbin doesn't require auth — use any header to prove injection)
ALTER TABLE upstream_credentials
    ADD CONSTRAINT upstream_credentials_partner_header_key
    UNIQUE (partner_id, header_name);
INSERT INTO upstream_credentials (partner_id, header_name, header_value)
SELECT id, 'X-Upstream-Key', 'demo-upstream-secret'
FROM partners
WHERE name = 'example'
ON CONFLICT (partner_id, header_name) DO NOTHING;
