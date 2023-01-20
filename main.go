package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"strconv"

	"github.com/breez/swapd/app"
	"github.com/breez/swapd/postgresql"
	"github.com/breez/swapd/server"
	"github.com/breez/swapd/swap"
	"github.com/btcsuite/btcd/chaincfg"
)

func main() {
	defaultLockTime, err := strconv.ParseUint(
		os.Getenv("DEFAULT_LOCK_TIME"),
		10,
		32,
	)
	if err != nil {
		log.Fatalf("Failed to parse DEFAULT_LOCK_TIME: %v", err)
	}

	network := os.Getenv("NETWORK")
	params, err := getNetParams(network)
	if err != nil {
		log.Fatalf("Failed to parse NETWORK: %v", err)
	}
	swapService := swap.NewSwapService(params, uint32(defaultLockTime))

	databaseUrl := os.Getenv("DATABASE_URL")
	swapRepository := postgresql.NewSwapRepository(databaseUrl)
	err = swapRepository.Connect(context.Background())
	if err != nil {
		log.Fatalf("Failed to connect to postgresql: %v", err)
	}

	swapper := app.NewSwapper(swapRepository, swapService)
	listenAddress := os.Getenv("LISTEN_ADDRESS")
	server := server.NewSwapServer(listenAddress, swapper)

	err = server.Start()
	if err == nil {
		log.Printf("Server stopped.")
	} else {
		log.Fatalf("Server stopped with error: %v", err)
	}
}

func getNetParams(network string) (*chaincfg.Params, error) {
	switch network {
	case "mainnet":
		return &chaincfg.MainNetParams, nil
	case "testnet":
		return &chaincfg.TestNet3Params, nil
	case "simnet":
		return &chaincfg.SimNetParams, nil
	case "signet":
		return &chaincfg.SigNetParams, nil
	case "regtest":
		return &chaincfg.RegressionNetParams, nil
	default:
		return nil, fmt.Errorf("invalid network '%s'", network)
	}
}
