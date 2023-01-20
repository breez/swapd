package swap

import "fmt"

// ValidatePaymentHash validates the paymenthash format.
func ValidatePaymentHash(paymentHash []byte) error {
	if len(paymentHash) != 32 {
		return fmt.Errorf("paymentHash length must be 32 bytes")
	}

	return nil
}

// ValidatePubKey validates the pubKey format.
func ValidatePubKey(pubKey []byte) error {
	if len(pubKey) != 33 {
		return fmt.Errorf("pubKey length must be 33 bytes")
	}

	return nil
}
