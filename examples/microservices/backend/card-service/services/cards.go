package services

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"

	// ── Veld SDKs: typed clients for consumed services ────────────────
	// Auto-generated from iam.veld and accounts.veld — inter-service calls.
	accountssdk "example.com/veld-generated/sdk/accounts"
	iamsdk "example.com/veld-generated/sdk/iam"
)

// SDK clients — in production, forward auth headers per-request.
var (
	iamClient      = iamsdk.NewClient("")      // defaults to VELD_IAM_URL
	accountsClient = accountssdk.NewClient("") // defaults to VELD_ACCOUNTS_URL
)

// CardsService satisfies the Veld-generated interfaces.CardsService interface.
type CardsService struct {
	mu    sync.RWMutex
	store []models.Card
}

func NewCardsService() *CardsService {
	return &CardsService{
		store: []models.Card{
			{
				Id: "card-001", AccountId: "acc-001", UserId: "user-001",
				Type: "debit", Status: "active", Network: "visa",
				Last4: "4242", ExpiresAt: "04/27", CreatedAt: "2024-01-15T09:00:00Z",
			},
		},
	}
}

func (s *CardsService) ListCards(ctx context.Context) ([]models.Card, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make([]models.Card, len(s.store))
	copy(result, s.store)
	return result, nil
}

func (s *CardsService) GetCard(ctx context.Context, id string) (*models.Card, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, c := range s.store {
		if c.Id == id {
			c := c
			return &c, nil
		}
	}
	return nil, fmt.Errorf("card %s not found", id)
}

func (s *CardsService) RequestCard(ctx context.Context, input *models.RequestCardInput) (*models.Card, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// ── SDK call: verify the target account exists via Accounts service ──
	_, err := accountsClient.GetAccount(ctx, input.AccountId)
	if err != nil {
		return nil, fmt.Errorf("cannot issue card: account %s not found (%w)", input.AccountId, err)
	}

	// ── SDK call: verify user identity via IAM service ───────────────────
	_, err = iamClient.GetProfile(ctx)
	if err != nil {
		return nil, fmt.Errorf("cannot issue card: failed to verify user identity (%w)", err)
	}

	card := models.Card{
		Id:        uuid.NewString(),
		AccountId: input.AccountId,
		UserId:    "from-auth-middleware",
		Type:      input.Type,
		Status:    "active",
		Network:   input.Network,
		Last4:     "9999",
		ExpiresAt: time.Now().AddDate(3, 0, 0).Format("01/06"),
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
	}
	s.store = append(s.store, card)
	return &card, nil
}

func (s *CardsService) FreezeCard(ctx context.Context, id string) (*models.Card, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, c := range s.store {
		if c.Id == id {
			s.store[i].Status = "frozen"
			c := s.store[i]
			return &c, nil
		}
	}
	return nil, fmt.Errorf("card %s not found", id)
}

func (s *CardsService) CancelCard(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, c := range s.store {
		if c.Id == id {
			s.store[i].Status = "cancelled"
			return nil
		}
	}
	return errors.New("card not found")
}
