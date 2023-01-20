CREATE TABLE swaps (
    id serial PRIMARY KEY,
	payment_hash bytea UNIQUE NOT NULL,
	payer_pubkey bytea NOT NULL,
    service_pubkey bytea NOT NULL,
    service_privkey bytea NOT NULL,
    lock_time integer NOT NULL,
    script bytea NOT NULL,
    'address' varchar NOT NULL
);
