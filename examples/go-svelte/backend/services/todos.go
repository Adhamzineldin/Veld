package services

import (
	"context"
	"errors"
	"fmt"
	"sync"

	"example.com/veld-generated/internal/models"
)

// TodosService is an in-memory implementation of interfaces.TodosService.
type TodosService struct {
	mu    sync.RWMutex
	todos []models.Todo
	next  int
}

func (s *TodosService) ListTodos(ctx context.Context) ([]models.Todo, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make([]models.Todo, len(s.todos))
	copy(result, s.todos)
	return result, nil
}

func (s *TodosService) GetTodo(ctx context.Context, id string) (*models.Todo, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, t := range s.todos {
		if t.Id == id {
			t := t
			return &t, nil
		}
	}
	return nil, errors.New("todo not found")
}

func (s *TodosService) CreateTodo(ctx context.Context, input *models.CreateTodoInput) (*models.Todo, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.next++
	t := models.Todo{
		Id:        fmt.Sprintf("%d", s.next),
		Title:     input.Title,
		Completed: false,
		UserId:    input.UserId,
	}
	s.todos = append(s.todos, t)
	return &t, nil
}

func (s *TodosService) UpdateTodo(ctx context.Context, id string, input *models.UpdateTodoInput) (*models.Todo, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, t := range s.todos {
		if t.Id == id {
			if input.Title != nil {
				s.todos[i].Title = *input.Title
			}
			if input.Completed != nil {
				s.todos[i].Completed = *input.Completed
			}
			updated := s.todos[i]
			return &updated, nil
		}
	}
	return nil, errors.New("todo not found")
}

func (s *TodosService) DeleteTodo(ctx context.Context, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, t := range s.todos {
		if t.Id == id {
			s.todos = append(s.todos[:i], s.todos[i+1:]...)
			return nil
		}
	}
	return errors.New("todo not found")
}
