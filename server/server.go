package server

import (
	"context"
	"fmt"
	"log"
	"net"
	"sync"

	"github.com/breez/swapd/app"
	"github.com/breez/swapd/rpc"
	"google.golang.org/grpc"
)

type SwapServer struct {
	rpc.SwapServer
	address string
	server  *grpc.Server
	swapper *app.Swapper
	mtx     sync.Mutex
}

func NewSwapServer(
	address string,
	swapper *app.Swapper,
) *SwapServer {
	return &SwapServer{
		address: address,
		swapper: swapper,
	}
}

func (s *SwapServer) Start() error {
	s.mtx.Lock()
	if s.server != nil {
		s.mtx.Unlock()
		return fmt.Errorf("server already started")
	}

	lis, err := net.Listen("tcp", s.address)
	if err != nil {
		s.mtx.Unlock()
		return fmt.Errorf("failed to listen: %v", err)
	}

	s.server = grpc.NewServer()
	s.mtx.Unlock()

	log.Printf("Swap server starting to listen on '%s'.", s.address)
	return s.server.Serve(lis)
}

func (s *SwapServer) Stop() {
	s.mtx.Lock()
	defer s.mtx.Unlock()

	if s.server == nil {
		return
	}

	log.Printf("Swap server stopping.")
	s.server.GracefulStop()
	s.server = nil
	log.Printf("Swap server stopped.")
}

func (s *SwapServer) InitSwap(
	ctx context.Context,
	request *rpc.InitSwapRequest,
) (*rpc.InitSwapResponse, error) {
	swap, err := s.swapper.InitializeSwap(ctx, request.Pubkey, request.Hash)
	if err != nil {
		return nil, err
	}

	return &rpc.InitSwapResponse{
		Address:  swap.Address,
		Pubkey:   swap.ServicePubKey,
		LockTime: swap.LockTime,
	}, nil
}
