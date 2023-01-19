package swap

import (
	"crypto/sha256"
	"fmt"

	"github.com/btcsuite/btcd/btcec/v2"
	"github.com/btcsuite/btcd/btcutil"
	"github.com/btcsuite/btcd/chaincfg"
	"github.com/btcsuite/btcd/txscript"
	"golang.org/x/crypto/ripemd160"
)

// SwapService offers functions to create and validate swaps.
type SwapService struct {
	defaultLockTime uint32
	net             *chaincfg.Params
}

// Initializes a new instance of the SwapService with the specified chain
// parameters and default lock time.
func NewSwapService(
	net *chaincfg.Params,
	defaultLockTime uint32,
) *SwapService {
	return &SwapService{
		defaultLockTime: defaultLockTime,
		net:             net,
	}
}

// SwapPublicInfo contains the swap information that is known to both the payer
// and the service.
type SwapPublicInfo struct {
	LockTime      uint32
	Script        []byte
	PayerPubKey   []byte
	ServicePubKey []byte
	PaymentHash   []byte
	Address       string
}

// SwapPrivateInfo contains the swap information that is known only to the
// service
type SwapPrivateInfo struct {
	ServicePrivKey []byte
}

// NewSubmarineSwap constructs a new submarine swap address, given the
// provided payerPubKey and paymentHash. It creates a new servicePrivKey as a
// spending path in the redeem script.
func (s *SwapService) NewSubmarineSwap(
	payerPubKey,
	paymentHash []byte,
) (*SwapPublicInfo, *SwapPrivateInfo, error) {
	err := ValidatePubKey(payerPubKey)
	if err != nil {
		return nil, nil, fmt.Errorf("invalid payerPubkey: %v", err)
	}

	err = ValidatePaymentHash(paymentHash)
	if err != nil {
		return nil, nil, fmt.Errorf("invalid paymentHash: %v", err)
	}

	// Create a new serviceKey
	serviceKey, err := btcec.NewPrivateKey()
	if err != nil {
		return nil, nil, fmt.Errorf(
			"failed to create serviceKey: %w",
			err,
		)
	}

	servicePrivKey := serviceKey.Serialize()
	servicePubKey := serviceKey.PubKey().SerializeCompressed()

	//Create the script
	script, err := genSubmarineSwapScript(
		servicePubKey,
		payerPubKey,
		paymentHash,
		int64(s.defaultLockTime),
	)
	if err != nil {
		return nil, nil, fmt.Errorf(
			"failed to generate swap script: %w",
			err,
		)
	}

	// Convert script to p2wsh address
	witnessProg := sha256.Sum256(script)
	address, err := btcutil.NewAddressWitnessScriptHash(
		witnessProg[:],
		s.net,
	)
	if err != nil {
		return nil, nil, fmt.Errorf(
			"failed to create p2wsh address: %w",
			err,
		)
	}

	return &SwapPublicInfo{
			LockTime:      s.defaultLockTime,
			Script:        script,
			PayerPubKey:   payerPubKey,
			ServicePubKey: servicePubKey,
			PaymentHash:   paymentHash,
			Address:       address.String(),
		}, &SwapPrivateInfo{
			ServicePrivKey: servicePrivKey,
		}, nil
}

// genSubmarineSwapScript generates the script for a submarine swap.
func genSubmarineSwapScript(
	servicePubKey,
	payerPubKey,
	hash []byte,
	lockTime int64,
) ([]byte, error) {
	builder := txscript.NewScriptBuilder()

	// Hash the preimage and check whether it matches the payment hash.
	builder.AddOp(txscript.OP_HASH160)
	builder.AddData(ripemd160H(hash))

	// Leaves 0P1 (true) on the stack if preimage matches
	builder.AddOp(txscript.OP_EQUAL)

	// Path taken if preimage matches, meaning the service has successfully
	// paid out the swap offchain and obtained the preimage to release the
	// onchain funds.
	builder.AddOp(txscript.OP_IF)
	builder.AddData(servicePubKey)

	// Refund back to payer. The payer can get a refund after the locktime
	// has expired.
	builder.AddOp(txscript.OP_ELSE)
	builder.AddInt64(lockTime)
	builder.AddOp(txscript.OP_CHECKSEQUENCEVERIFY)
	builder.AddOp(txscript.OP_DROP)
	builder.AddData(payerPubKey)

	builder.AddOp(txscript.OP_ENDIF)

	// Checksig checks either the payer or service sig depending on the path
	// taken.
	builder.AddOp(txscript.OP_CHECKSIG)

	return builder.Script()
}

// ripemd160H calculates the ripemd160 of the passed byte slice. This is used to
// calculate the intermediate hash for payment pre-images. Payment hashes are
// the result of ripemd160(sha256(paymentPreimage)). As a result, the value
// passed in should be the sha256 of the payment hash.
func ripemd160H(d []byte) []byte {
	h := ripemd160.New()
	h.Write(d)
	return h.Sum(nil)
}
