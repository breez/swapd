package postgresql

import (
	"context"
	"fmt"
	"sync"

	"github.com/breez/swapd/data"
	"github.com/breez/swapd/swap"
	"github.com/jackc/pgx/v4"
	"github.com/jackc/pgx/v4/pgxpool"
)

type SwapRepository struct {
	data.SwapRepository
	databaseUrl string
	pool        *pgxpool.Pool
	mtx         sync.Mutex
}

func NewSwapRepository(databaseUrl string) *SwapRepository {
	return &SwapRepository{
		databaseUrl: databaseUrl,
	}
}

func (s *SwapRepository) Connect(ctx context.Context) error {
	return s.ensureConnected(ctx)
}

func (s *SwapRepository) GetSwap(
	ctx context.Context,
	paymentHash []byte,
) (*swap.SwapPublicInfo, error) {
	err := s.ensureConnected(ctx)
	if err != nil {
		return nil, err
	}

	row := s.pool.QueryRow(ctx,
		`SELECT payer_pubkey, service_pubkey, script, lock_time
		, 'address'
		 FROM swaps
		 WHERE payment_hash = $1`,
		paymentHash,
	)

	var (
		payerPubKey   []byte
		servicePubKey []byte
		script        []byte
		address       string
		lockTime      int32
	)
	err = row.Scan(
		&payerPubKey, &servicePubKey, &script, &lockTime, &address)
	if err == pgx.ErrNoRows {
		return nil, nil
	}

	return &swap.SwapPublicInfo{
		LockTime:      uint32(lockTime),
		Script:        script,
		PayerPubKey:   payerPubKey,
		ServicePubKey: servicePubKey,
		PaymentHash:   paymentHash,
		Address:       address,
	}, nil
}

func (s *SwapRepository) AddSwap(
	ctx context.Context,
	public *swap.SwapPublicInfo,
	private *swap.SwapPrivateInfo,
) error {
	err := s.ensureConnected(ctx)
	if err != nil {
		return err
	}

	tag, err := s.pool.Exec(ctx,
		`INSERT INTO swaps (payment_hash, payer_pubkey, service_pubkey, 
			service_privkey, lock_time, script, address)
		 VALUES ($1, $2, $3, $4, $5, $6)
		 ON CONFLICT DO NOTHING`,
		public.PaymentHash,
		public.PayerPubKey,
		public.ServicePubKey,
		private.ServicePrivKey,
		public.LockTime,
		public.Script,
		public.Address,
	)

	if err != nil {
		return err
	}

	if tag.RowsAffected() == 0 {
		return fmt.Errorf("swap already exists")
	}

	return nil
}

func (s *SwapRepository) ensureConnected(ctx context.Context) error {
	if s.pool != nil {
		return nil
	}

	s.mtx.Lock()
	defer s.mtx.Unlock()
	if s.pool != nil {
		return nil
	}

	pool, err := pgxpool.Connect(ctx, s.databaseUrl)
	if err != nil {
		return fmt.Errorf(
			"failed to connect to %s: %w",
			s.databaseUrl,
			err,
		)
	}

	s.pool = pool
	return nil
}
