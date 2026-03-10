package handlers

import (
	"encoding/json"
	"net/http"
	"time"

	serverauth "github.com/Adhamzineldin/Veld/internal/server/auth"
	"github.com/Adhamzineldin/Veld/internal/server/db"
	"github.com/Adhamzineldin/Veld/internal/server/models"
	"golang.org/x/crypto/bcrypt"
)

// AuthHandler handles authentication endpoints.
type AuthHandler struct {
	DB        *db.DB
	JWTSecret string
}

type registerBody struct {
	Email    string `json:"email"`
	Username string `json:"username"`
	Password string `json:"password"`
}

type loginBody struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

// Register handles POST /api/v1/auth/register
func (h *AuthHandler) Register(w http.ResponseWriter, r *http.Request) {
	var body registerBody
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		jsonError(w, "invalid request body", http.StatusBadRequest)
		return
	}
	if body.Email == "" || body.Username == "" || body.Password == "" {
		jsonError(w, "email, username and password are required", http.StatusBadRequest)
		return
	}
	if len(body.Password) < 8 {
		jsonError(w, "password must be at least 8 characters", http.StatusBadRequest)
		return
	}

	hash, err := bcrypt.GenerateFromPassword([]byte(body.Password), bcrypt.DefaultCost)
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}

	u := &models.User{
		ID:           serverauth.GenerateID(),
		Email:        body.Email,
		Username:     body.Username,
		PasswordHash: string(hash),
		CreatedAt:    time.Now().UTC(),
	}
	if err := h.DB.CreateUser(u); err != nil {
		jsonError(w, "email or username already taken", http.StatusConflict)
		return
	}

	jwt, err := serverauth.IssueJWT(u.ID, h.JWTSecret, 7*24*time.Hour)
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	setSessionCookie(w, jwt)
	jsonCreated(w, map[string]interface{}{"user": u, "token": jwt})
}

// Login handles POST /api/v1/auth/login
func (h *AuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	var body loginBody
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		jsonError(w, "invalid request body", http.StatusBadRequest)
		return
	}

	u, err := h.DB.GetUserByEmail(body.Email)
	if err != nil || u == nil {
		jsonError(w, "invalid credentials", http.StatusUnauthorized)
		return
	}
	if bcrypt.CompareHashAndPassword([]byte(u.PasswordHash), []byte(body.Password)) != nil {
		jsonError(w, "invalid credentials", http.StatusUnauthorized)
		return
	}

	jwt, err := serverauth.IssueJWT(u.ID, h.JWTSecret, 7*24*time.Hour)
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	setSessionCookie(w, jwt)
	jsonOK(w, map[string]interface{}{"user": u, "token": jwt})
}

// Logout handles POST /api/v1/auth/logout
func (h *AuthHandler) Logout(w http.ResponseWriter, r *http.Request) {
	http.SetCookie(w, &http.Cookie{
		Name:     "veld_session",
		Value:    "",
		MaxAge:   -1,
		Path:     "/",
		HttpOnly: true,
	})
	jsonOK(w, map[string]string{"message": "logged out"})
}

// Me handles GET /api/v1/auth/me
func (h *AuthHandler) Me(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	jsonOK(w, u)
}

func setSessionCookie(w http.ResponseWriter, jwt string) {
	http.SetCookie(w, &http.Cookie{
		Name:     "veld_session",
		Value:    jwt,
		MaxAge:   int((7 * 24 * time.Hour).Seconds()),
		Path:     "/",
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
	})
}

// ── Token endpoints ────────────────────────────────────────────────────────────

type createTokenBody struct {
	Name    string   `json:"name"`
	Scopes  []string `json:"scopes"`
	OrgID   string   `json:"org_id"`
	ExpDays int      `json:"expires_days"` // 0 = never
}

// CreateToken handles POST /api/v1/tokens
func (h *AuthHandler) CreateToken(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	var body createTokenBody
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		jsonError(w, "invalid body", http.StatusBadRequest)
		return
	}
	if body.Name == "" {
		jsonError(w, "name is required", http.StatusBadRequest)
		return
	}
	if len(body.Scopes) == 0 {
		body.Scopes = []string{"read"}
	}

	plain, hash, err := serverauth.GenerateToken()
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}

	now := time.Now().UTC()
	tok := &models.Token{
		ID:        serverauth.GenerateID(),
		UserID:    u.ID,
		OrgID:     body.OrgID,
		Name:      body.Name,
		TokenHash: hash,
		Scopes:    body.Scopes,
		CreatedAt: now,
	}
	if body.ExpDays > 0 {
		exp := now.AddDate(0, 0, body.ExpDays)
		tok.ExpiresAt = &exp
	}

	if err := h.DB.CreateToken(tok); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	tok.PlainToken = plain
	jsonCreated(w, tok)
}

// ListTokens handles GET /api/v1/tokens
func (h *AuthHandler) ListTokens(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	tokens, err := h.DB.ListTokensForUser(u.ID)
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	if tokens == nil {
		tokens = []*models.Token{}
	}
	jsonOK(w, tokens)
}

// DeleteToken handles DELETE /api/v1/tokens/{id}
func (h *AuthHandler) DeleteToken(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	id := r.PathValue("id")
	if err := h.DB.DeleteToken(id, u.ID); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	jsonNoContent(w)
}

// Stats handles GET /api/v1/stats
func (h *AuthHandler) Stats(w http.ResponseWriter, r *http.Request) {
	s, err := h.DB.GetStats()
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	jsonOK(w, s)
}
