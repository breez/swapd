from helpers import *
import musig2
import os
from bitcoinutils.transactions import Locktime
from bitcoinutils.ripemd160 import ripemd160
from bitcoinutils.script import Script, OP_CODES
from bitcoinutils.utils import get_tag_hashed_merkle_root, tagged_hash
from bitcoinutils.setup import setup
from bitcoinutils.keys import P2trAddress, PublicKey
from bitcoinutils.transactions import Transaction, TxInput, TxOutput, TxWitnessInput
from decimal import Decimal


def test_cooperative_refund(node_factory, swapd_factory):
    setup("regtest")
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, payment_request, h, refund_privkey, claim_pubkey, lock_height = (
        create_swap_no_invoice_extended(user, swapper)
    )
    to_spend_txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)
    to_spend_tx = user.bitcoin.rpc.getrawtransaction(to_spend_txid, True)
    to_spend_output_index = 0
    if to_spend_tx["vout"][1]["value"] == Decimal("0.00100000"):
        to_spend_output_index = 1

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    refund_address = P2trAddress(user.new_address())
    extra_in = os.urandom(32)

    tx_in = TxInput(to_spend_txid, to_spend_output_index)
    tx_out = TxOutput(99_000, refund_address.to_script_pub_key())
    tx = Transaction([tx_in], [tx_out], has_segwit=True, witnesses=[TxWitnessInput([])])
    tx_digest = tx.get_transaction_taproot_digest(
        0, [P2trAddress(address).to_script_pub_key()], [100_000]
    )
    refund_pubkey_bytes = bytes.fromhex(refund_privkey.get_public_key().to_hex())
    pubkeys = musig2.key_sort([claim_pubkey, refund_pubkey_bytes])
    agg_ctx = musig2.key_agg(pubkeys)
    aggpk = musig2.get_xonly_pk(agg_ctx)
    refund_privkey_bytes = refund_privkey.to_bytes()

    scripts = [
        Script(
            [
                "OP_HASH160",
                ripemd160(bytes.fromhex(h)).hex(),
                "OP_EQUALVERIFY",
                PublicKey(claim_pubkey.hex()).to_x_only_hex(),
                "OP_CHECKSIG",
            ]
        ),
        Script(
            [
                refund_privkey.get_public_key().to_x_only_hex(),
                "OP_CHECKSIGVERIFY",
                lock_height,
                "OP_CHECKLOCKTIMEVERIFY",
            ]
        ),
    ]
    merkle_root = get_tag_hashed_merkle_root(scripts)
    tweak = tagged_hash(aggpk + merkle_root, "TapTweak")
    tweaked_internal_key = musig2.get_xonly_pk(
        musig2.apply_tweak(agg_ctx, tweak, is_xonly=True)
    )
    secnonce, pubnonce = musig2.nonce_gen(
        refund_privkey_bytes,
        refund_pubkey_bytes,
        tweaked_internal_key,
        tx_digest,
        extra_in,
    )
    resp = swapper.rpc.refund_swap(address, tx.to_bytes(has_segwit=False), 0, pubnonce)
    their_partial_sig = resp.partial_signature
    their_pub_nonce = resp.pub_nonce
    agg_nonce = musig2.nonce_agg([their_pub_nonce, pubnonce])
    session = musig2.SessionContext(agg_nonce, pubkeys, [tweak], [True], tx_digest)
    our_partial_sig = musig2.sign(secnonce, refund_privkey_bytes, session)

    sig_agg = musig2.partial_sig_agg([their_partial_sig, our_partial_sig], session)
    tx.witnesses[0].stack.append(sig_agg.hex())

    expected_utxos = len(user.list_utxos()) + 1
    user.bitcoin.rpc.sendrawtransaction(tx.to_hex())
    user.bitcoin.generate_block(1)
    wait_for(lambda: len(user.list_utxos()) == expected_utxos)
