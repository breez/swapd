CREATE TABLE swaps (
    id BIGSERIAL PRIMARY KEY,
    creation_time BIGINT NOT NULL,
    payer_pubkey BYTEA NOT NULL,
    swapper_pubkey BYTEA NOT NULL,
    payment_hash BYTEA NOT NULL,
    script BYTEA NOT NULL, -- TODO: remove?
    address VARCHAR NOT NULL,
    lock_time BIGINT NOT NULL,
    swapper_privkey BYTEA NOT NULL, -- TODO: encrypt?
    preimage BYTEA NULL
);

CREATE UNIQUE INDEX swaps_payment_hash_key ON swaps (payment_hash);

-- Allow quick lookups by onchain address.
CREATE INDEX swaps_address_idx ON swaps (address);

CREATE TABLE blocks (
    block_hash varchar PRIMARY KEY,
    prev_block_hash varchar NOT NULL,
    height BIGINT NOT NULL
);

CREATE TABLE swap_utxos (
    swap_id BIGINT NOT NULL REFERENCES swaps,
    tx_id varchar NOT NULL,
    output_index bigint NOT NULL,
    amount bigint NOT NULL,
    block_hash varchar NOT NULL, -- not a foreign key, to keep a record of potentially lost coins due to reorgs.
    PRIMARY KEY (tx_id, output_index)
);

-- Allow easy tracking of which utxos to delete in case of a reorg.
CREATE INDEX swap_utxos_block_hash_idx ON swap_utxos(block_hash);

CREATE TABLE filter_addresses (
    address VARCHAR PRIMARY KEY
);