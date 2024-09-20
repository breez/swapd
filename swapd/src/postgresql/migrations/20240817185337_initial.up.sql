
/*
    swaps
*/

CREATE TABLE swaps (
    payment_hash BYTEA NOT NULL PRIMARY KEY,
    creation_time BIGINT NOT NULL,
    payer_pubkey BYTEA NOT NULL,
    swapper_pubkey BYTEA NOT NULL,
    script BYTEA NOT NULL, -- TODO: remove?
    address VARCHAR NOT NULL,
    lock_time BIGINT NOT NULL,
    swapper_privkey BYTEA NOT NULL, -- TODO: encrypt?
    preimage BYTEA NULL
);

CREATE INDEX swaps_address_idx ON swaps (address);

CREATE TABLE payment_attempts (
    id BIGSERIAL PRIMARY KEY,
    swap_payment_hash BYTEA NOT NULL REFERENCES swaps,
    label VARCHAR NOT NULL,
    creation_time BIGINT NOT NULL,
    amount_msat BIGINT NOT NULL,
    payment_request VARCHAR NOT NULL,
    destination BYTEA NOT NULL,
    success BOOLEAN NULL,
    error VARCHAR NULL
);

CREATE INDEX payment_attempts_swap_payment_hash_idx ON payment_attempts(swap_payment_hash);
CREATE INDEX payment_attempts_label_idx ON payment_attempts(label);
CREATE INDEX payment_attempts_payment_request_idx ON payment_attempts(payment_request);
CREATE INDEX payment_attempts_destination_idx ON payment_attempts(destination);

CREATE TABLE payment_attempt_tx_outputs (
    payment_attempt_id BIGINT NOT NULL REFERENCES payment_attempts,
    tx_id VARCHAR NOT NULL,
    output_index BIGINT NOT NULL
);

CREATE INDEX payment_attempt_tx_outputs_payment_attempt_id_idx
ON payment_attempt_tx_outputs (payment_attempt_id);
CREATE INDEX payment_attempt_tx_outputs_tx_id_output_index_idx
ON payment_attempt_tx_outputs (tx_id, output_index);

/*
    chain
*/
CREATE TABLE blocks (
    block_hash varchar PRIMARY KEY,
    prev_block_hash varchar NOT NULL,
    height BIGINT NOT NULL
);

CREATE INDEX blocks_height_idx ON blocks(height);

CREATE TABLE watch_addresses (
    address VARCHAR PRIMARY KEY
);

CREATE TABLE tx_outputs (
    tx_id VARCHAR NOT NULL,
    output_index BIGINT NOT NULL,
    address VARCHAR NOT NULL,
    amount BIGINT NOT NULL,
    PRIMARY KEY (tx_id, output_index)
);

CREATE INDEX tx_outputs_address_idx ON tx_outputs(address);

-- The trick with tx_blocks is when a block is removed, the tx here is also
-- removed. That's the mechanism to handle reorgs. Always check the tx_blocks
-- table if looking for a confirmed utxo.
CREATE TABLE tx_blocks (
    tx_id VARCHAR NOT NULL,
    block_hash VARCHAR NOT NULL,
    FOREIGN KEY (block_hash) REFERENCES blocks (block_hash) ON DELETE CASCADE
);

CREATE INDEX tx_blocks_tx_id_output_index_idx ON tx_blocks(tx_id);
CREATE INDEX tx_blocks_block_hash_idx ON tx_blocks(block_hash);

-- tx_inputs spend tx_outputs
CREATE TABLE tx_inputs (
    tx_id VARCHAR NOT NULL,
    output_index BIGINT NOT NULL,
    spending_tx_id VARCHAR NOT NULL,
    spending_input_index BIGINT NOT NULL,
    PRIMARY KEY (spending_tx_id, spending_input_index),
    FOREIGN KEY (tx_id, output_index) REFERENCES tx_outputs (tx_id, output_index) ON DELETE CASCADE
);

CREATE INDEX tx_inputs_tx_id_output_index_idx ON tx_inputs(tx_id, output_index);

/*
    chain filter
*/
CREATE TABLE filter_addresses (
    address VARCHAR PRIMARY KEY
);

/*
    redeem
*/
CREATE TABLE redeems (
    tx_id VARCHAR NOT NULL PRIMARY KEY,
    creation_time BIGINT NOT NULL,
    tx bytea NOT NULL,
    destination_address VARCHAR NOT NULL,
    fee_per_kw BIGINT NOT NULL
);

CREATE TABLE redeem_inputs (
    redeem_tx_id VARCHAR NOT NULL REFERENCES redeems,
    tx_id VARCHAR NOT NULL,
    output_index BIGINT NOT NULL
);
CREATE INDEX redeem_inputs_redeem_tx_id_idx ON redeem_inputs(redeem_tx_id);
