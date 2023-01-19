package data

import (
	"context"

	"github.com/breez/swapd/swap"
)

// SwapRepository contains functions to persists and retrieve swap information.
type SwapRepository interface {
	// GetSwap gets the public swap information from the data store.
	GetSwap(
		ctx context.Context,
		paymentHash []byte,
	) (*swap.SwapPublicInfo, error)

	// AddSwap adds the swap information to the data store. Should fail with
	// an error if the paymentHash already exists.
	AddSwap(
		ctx context.Context,
		public *swap.SwapPublicInfo,
		private *swap.SwapPrivateInfo,
	) error
}
