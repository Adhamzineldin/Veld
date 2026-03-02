package services

import (
	"context"
	"errors"
	"fmt"
	"sync"

	"example.com/veld-generated/internal/models"
)

// UsersService is an in-memory implementation of interfaces.UsersService.
type UsersService struct {
	mu    sync.RWMutex
	users []models.User
	next  int
}

func (s *UsersService) ListUsers(ctx context.Context) ([]models.User, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make([]models.User, len(s.users))
	copy(result, s.users)
	return result, nil
}

func (s *UsersService) GetUser(ctx context.Context, id string) (*models.User, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, u := range s.users {
		if u.Id == id {
			u := u
			return &u, nil
		}
	}
	return nil, errors.New("user not found")
}

func (s *UsersService) CreateUser(ctx context.Context, input *models.CreateUserInput) (*models.User, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.next++
	u := models.User{
		Id:    fmt.Sprintf("%d", s.next),
		Name:  input.Name,
		Email: input.Email,
	}
	s.users = append(s.users, u)
	return &u, nil
}

func (s *UsersService) DeleteUser(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, u := range s.users {
		if u.Id == id {
			s.users = append(s.users[:i], s.users[i+1:]...)
			return nil
		}
	}
	return errors.New("user not found")
}
