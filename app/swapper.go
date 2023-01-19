package app

import (
	"context"
	"fmt"

	"github.com/breez/swapd/data"
	"github.com/breez/swapd/swap"
)

// Swapper contains top level application layer functions to manage swaps.
type Swapper struct {
	swapRepository data.SwapRepository
	swapService    *swap.SwapService
}

func NewSwapper(
	swapRepository data.SwapRepository,
	swapService *swap.SwapService,
) *Swapper {
	return &Swapper{
		swapRepository: swapRepository,
		swapService:    swapService,
	}
}

// InitializeSwap creates a new swap and persists it to the data store.
func (s *Swapper) InitializeSwap(
	ctx context.Context,
	payerPubkey []byte,
	paymentHash []byte,
) (*swap.SwapPublicInfo, error) {
	// See if a swap with the given payment hash already exists.
	swap, err := s.swapRepository.GetSwap(ctx, paymentHash)
	if err != nil {
		return nil, fmt.Errorf("error retrieving swap: %v", err)
	}
	if swap != nil {
		return nil, fmt.Errorf("swap already in progress")
	}

	// Construct the swap information.
	public, private, err := s.swapService.NewSubmarineSwap(
		payerPubkey,
		paymentHash,
	)
	if err != nil {
		return nil, fmt.Errorf("failed to create swap info: %v", err)
	}

	// Persist the swap.
	err = s.swapRepository.AddSwap(ctx, public, private)
	if err != nil {
		// TODO: Check for swap already exists and return same error as
		// above 'swap already in progress'.
		return nil, fmt.Errorf("failed to add swap: %v", err)
	}

	// Return the swap.
	return public, nil
}
