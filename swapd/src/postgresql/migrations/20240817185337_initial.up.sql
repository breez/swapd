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

CREATE INDEX blocks_height_idx ON blocks(height);

CREATE TABLE watch_addresses (
    address VARCHAR PRIMARY KEY
);

CREATE TABLE address_utxos (
    id BIGSERIAL PRIMARY KEY, 
    address VARCHAR NOT NULL,
    tx_id VARCHAR NOT NULL,
    output_index BIGINT NOT NULL,
    amount BIGINT NOT NULL,
    block_hash VARCHAR NOT NULL -- not a foreign key, to keep a record of potentially lost coins due to reorgs.
);

CREATE INDEX address_utxos_tx_id_output_index_idx ON address_utxos(tx_id, output_index);
CREATE INDEX address_utxos_address_idx ON address_utxos(address);
-- Allow easy tracking of which utxos to delete in case of a reorg.
CREATE INDEX address_utxos_block_hash_idx ON address_utxos(block_hash);

CREATE TABLE spent_utxos (
    id BIGSERIAL PRIMARY KEY,
    utxo_id BIGINT NOT NULL REFERENCES address_utxos,
    spending_tx_id VARCHAR NOT NULL,
    spending_block_hash VARCHAR NOT NULL
);

CREATE TABLE filter_addresses (
    address VARCHAR PRIMARY KEY
);
