package handlers

import (
	"encoding/json"
	"log"
	"net/http"
	"strings"
	"time"

	serverauth "github.com/Adhamzineldin/Veld/internal/server/auth"
	"github.com/Adhamzineldin/Veld/internal/server/db"
	"github.com/Adhamzineldin/Veld/internal/server/email"
	"github.com/Adhamzineldin/Veld/internal/server/models"
	"golang.org/x/crypto/bcrypt"
)

// AuthHandler handles authentication endpoints.
type AuthHandler struct {
	DB        *db.DB
	JWTSecret string
	Email     email.Config
	BaseURL   string // e.g. "http://localhost:8080" — used for verification links
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

	// Send verification email asynchronously (non-blocking)
	if h.Email.Enabled() {
		log.Printf("[email] register: dispatching verification email for new user %s (%s)", u.Username, u.Email)
		go h.sendVerificationEmail(u)
	} else {
		log.Printf("[email] register: SMTP not configured — skipping verification email for %s", u.Email)
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

	// If TOTP is enabled, return partial session requiring second factor
	if u.TOTPEnabled {
		partial, err := serverauth.IssueJWT(u.ID, h.JWTSecret, 5*time.Minute)
		if err != nil {
			jsonError(w, "server error", http.StatusInternalServerError)
			return
		}
		jsonOK(w, map[string]interface{}{
			"totp_required": true,
			"partial":       partial,
		})
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

// TOTPLogin handles POST /api/v1/auth/totp-login — step 2 of 2FA
func (h *AuthHandler) TOTPLogin(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Partial string `json:"partial"` // short-lived JWT from Login step
		Code    string `json:"code"`    // 6-digit TOTP code
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		jsonError(w, "invalid request body", http.StatusBadRequest)
		return
	}
	claims, err := serverauth.VerifyJWT(body.Partial, h.JWTSecret)
	if err != nil {
		jsonError(w, "session expired — please log in again", http.StatusUnauthorized)
		return
	}
	u, err := h.DB.GetUserByID(claims.Sub)
	if err != nil || u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	if !u.TOTPEnabled || !serverauth.VerifyTOTP(u.TOTPSecret, body.Code) {
		jsonError(w, "invalid authenticator code", http.StatusUnauthorized)
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

// ── TOTP 2FA ──────────────────────────────────────────────────────────────────

// SetupTOTP handles POST /api/v1/auth/setup-totp
// Generates a pending secret and returns the otpauth URI for QR display.
func (h *AuthHandler) SetupTOTP(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	secret, err := serverauth.GenerateTOTPSecret()
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	if err := h.DB.SetPendingTOTPSecret(u.ID, secret); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	issuer := "Veld Registry"
	uri := serverauth.OTPAuthURI(secret, u.Username, issuer)
	jsonOK(w, map[string]string{
		"secret": secret,
		"uri":    uri,
	})
}

// ConfirmTOTP handles POST /api/v1/auth/confirm-totp
// Verifies a TOTP code against the pending secret, then enables TOTP.
func (h *AuthHandler) ConfirmTOTP(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	var body struct {
		Code string `json:"code"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil || body.Code == "" {
		jsonError(w, "code is required", http.StatusBadRequest)
		return
	}
	// Re-fetch to get pending secret (context user may be stale)
	fresh, err := h.DB.GetUserByID(u.ID)
	if err != nil || fresh == nil || fresh.PendingTOTPSecret == "" {
		jsonError(w, "no pending TOTP setup — call /api/v1/auth/setup-totp first", http.StatusBadRequest)
		return
	}
	if !serverauth.VerifyTOTP(fresh.PendingTOTPSecret, body.Code) {
		jsonError(w, "invalid code", http.StatusUnauthorized)
		return
	}
	if err := h.DB.ConfirmTOTP(u.ID); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	jsonOK(w, map[string]string{"message": "two-factor authentication enabled"})
}

// DisableTOTP handles DELETE /api/v1/auth/totp
// Requires password confirmation, then disables TOTP.
func (h *AuthHandler) DisableTOTP(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	var body struct {
		Password string `json:"password"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil || body.Password == "" {
		jsonError(w, "password is required", http.StatusBadRequest)
		return
	}
	fresh, err := h.DB.GetUserByID(u.ID)
	if err != nil || fresh == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	if bcrypt.CompareHashAndPassword([]byte(fresh.PasswordHash), []byte(body.Password)) != nil {
		jsonError(w, "incorrect password", http.StatusUnauthorized)
		return
	}
	if err := h.DB.DisableTOTP(u.ID); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	jsonOK(w, map[string]string{"message": "two-factor authentication disabled"})
}

// ── Email verification ─────────────────────────────────────────────────────────

// VerifyEmail handles POST /api/v1/auth/verify-email
func (h *AuthHandler) VerifyEmail(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Token string `json:"token"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil || body.Token == "" {
		jsonError(w, "token is required", http.StatusBadRequest)
		return
	}
	userID, expiresAt, err := h.DB.GetEmailVerificationToken(body.Token)
	if err != nil || userID == "" {
		jsonError(w, "invalid or expired verification link", http.StatusBadRequest)
		return
	}
	if time.Now().After(expiresAt) {
		h.DB.DeleteEmailVerificationToken(body.Token)
		jsonError(w, "verification link has expired — please request a new one", http.StatusBadRequest)
		return
	}
	if err := h.DB.SetEmailVerified(userID); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	h.DB.DeleteEmailVerificationToken(body.Token)

	// Send welcome email
	u, _ := h.DB.GetUserByID(userID)
	if u != nil && h.Email.Enabled() {
		log.Printf("[email] verify-email: sending welcome email to %s (%s)", u.Username, u.Email)
		go func() {
			if err := email.Send(h.Email, u.Email, "Welcome to Veld Registry!", email.WelcomeEmail(u.Username)); err != nil {
				log.Printf("[email] ERROR sending welcome email to %s: %v", u.Email, err)
			}
		}()
	}

	jsonOK(w, map[string]string{"message": "email verified"})
}

// ResendVerification handles POST /api/v1/auth/resend-verification
func (h *AuthHandler) ResendVerification(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		log.Printf("[email] resend-verification: no authenticated user")
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	log.Printf("[email] resend-verification: user=%s email=%s emailVerified=%v", u.Username, u.Email, u.EmailVerified)
	if u.EmailVerified {
		log.Printf("[email] resend-verification: email already verified for %s — rejecting", u.Email)
		jsonError(w, "email already verified", http.StatusBadRequest)
		return
	}
	if !h.Email.Enabled() {
		log.Printf("[email] resend-verification: SMTP not configured (host=%q from=%q) — returning 503", h.Email.Host, h.Email.From)
		jsonError(w, "email not configured on this registry", http.StatusServiceUnavailable)
		return
	}
	log.Printf("[email] resend-verification: dispatching verification email for %s", u.Email)
	go h.sendVerificationEmail(u)
	jsonOK(w, map[string]string{"message": "verification email sent"})
}

func (h *AuthHandler) sendVerificationEmail(u *models.User) {
	log.Printf("[email] sendVerificationEmail called for user=%s email=%s", u.Username, u.Email)

	if !h.Email.Enabled() {
		log.Printf("[email] SMTP not configured — skipping verification email for %s", u.Email)
		return
	}
	log.Printf("[email] SMTP config: host=%s port=%d from=%s username=%s",
		h.Email.Host, h.Email.Port, h.Email.From, h.Email.Username)

	tok, _ := serverauth.GenerateID(), ""
	tok = serverauth.GenerateID() + serverauth.GenerateID() // longer token
	tok = strings.ReplaceAll(tok, "-", "")
	expiresAt := time.Now().Add(24 * time.Hour)
	if err := h.DB.CreateEmailVerificationToken(tok, u.ID, expiresAt); err != nil {
		log.Printf("[email] ERROR creating verification token for %s: %v", u.Email, err)
		return
	}
	log.Printf("[email] verification token created for %s (expires %s)", u.Email, expiresAt.Format(time.RFC3339))

	base := strings.TrimRight(h.BaseURL, "/")
	if base == "" {
		base = "http://localhost:8080"
	}
	verifyURL := base + "/#/verify-email?token=" + tok
	log.Printf("[email] sending verification email to %s (verify URL: %s)", u.Email, verifyURL)

	if err := email.Send(h.Email, u.Email, "Verify your Veld Registry email", email.VerificationEmail(u.Username, verifyURL)); err != nil {
		log.Printf("[email] ERROR sending to %s: %v", u.Email, err)
	} else {
		log.Printf("[email] verification email sent successfully to %s", u.Email)
	}
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
