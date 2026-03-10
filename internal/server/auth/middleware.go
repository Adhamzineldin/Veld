package auth

import (
	"context"
	"net/http"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/server/db"
	"github.com/Adhamzineldin/Veld/internal/server/models"
)

type contextKey string

const (
	ContextUser  contextKey = "user"
	ContextToken contextKey = "token"
)

// Middleware authenticates requests via Bearer token or session JWT cookie.
// It sets ContextUser and ContextToken in the request context on success.
// Optional = true means unauthenticated requests are allowed through.
func Middleware(database *db.DB, jwtSecret string, optional bool) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			user, tok := resolveAuth(r, database, jwtSecret)
			if user == nil && !optional {
				http.Error(w, `{"error":"unauthorized"}`, http.StatusUnauthorized)
				return
			}
			ctx := r.Context()
			if user != nil {
				ctx = context.WithValue(ctx, ContextUser, user)
			}
			if tok != nil {
				ctx = context.WithValue(ctx, ContextToken, tok)
				database.TouchToken(tok.ID)
			}
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

// GetUser extracts the authenticated user from context (may be nil).
func GetUser(r *http.Request) *models.User {
	u, _ := r.Context().Value(ContextUser).(*models.User)
	return u
}

// GetToken extracts the API token from context (may be nil for JWT sessions).
func GetToken(r *http.Request) *models.Token {
	t, _ := r.Context().Value(ContextToken).(*models.Token)
	return t
}

// HasScope checks if the request token has the required scope.
// Always returns true for JWT (browser) sessions.
func HasScope(r *http.Request, scope string) bool {
	tok := GetToken(r)
	if tok == nil {
		return true // JWT session has all scopes
	}
	for _, s := range tok.Scopes {
		if s == scope || s == "admin" {
			return true
		}
	}
	return false
}

func resolveAuth(r *http.Request, database *db.DB, jwtSecret string) (*models.User, *models.Token) {
	// 1. Bearer header — could be a vtk_ API token or a JWT (CLI login)
	if auth := r.Header.Get("Authorization"); strings.HasPrefix(auth, "Bearer ") {
		plain := strings.TrimPrefix(auth, "Bearer ")

		// JWT if it contains two dots (header.payload.sig)
		if strings.Count(plain, ".") == 2 {
			claims, err := VerifyJWT(plain, jwtSecret)
			if err != nil {
				return nil, nil
			}
			user, err := database.GetUserByID(claims.Sub)
			if err != nil {
				return nil, nil
			}
			return user, nil // no Token record — treated as full-scope session
		}

		// vtk_ API token
		hash := HashToken(plain)
		tok, err := database.GetTokenByHash(hash)
		if err != nil || tok == nil {
			return nil, nil
		}
		user, err := database.GetUserByID(tok.UserID)
		if err != nil {
			return nil, nil
		}
		return user, tok
	}

	// 2. Session cookie (JWT — browser)
	cookie, err := r.Cookie("veld_session")
	if err != nil {
		return nil, nil
	}
	claims, err := VerifyJWT(cookie.Value, jwtSecret)
	if err != nil {
		return nil, nil
	}
	user, err := database.GetUserByID(claims.Sub)
	if err != nil {
		return nil, nil
	}
	return user, nil
}
