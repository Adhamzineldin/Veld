// Package auth handles token generation, hashing and JWT session cookies.
package auth

import (
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

const tokenPrefix = "vtk_"

// GenerateToken creates a new plain-text API token (vtk_<32 random bytes base64url>).
func GenerateToken() (plain, hash string, err error) {
	b := make([]byte, 32)
	if _, err = rand.Read(b); err != nil {
		return
	}
	plain = tokenPrefix + base64.RawURLEncoding.EncodeToString(b)
	hash = HashToken(plain)
	return
}

// HashToken returns the SHA-256 hex digest of a plain token for DB storage.
func HashToken(plain string) string {
	sum := sha256.Sum256([]byte(plain))
	return hex.EncodeToString(sum[:])
}

// GenerateID creates a random 16-byte hex ID.
func GenerateID() string {
	b := make([]byte, 16)
	rand.Read(b)
	return hex.EncodeToString(b)
}

// ── Minimal HMAC-SHA256 JWT (no external library) ─────────────────────────────

type jwtHeader struct {
	Alg string `json:"alg"`
	Typ string `json:"typ"`
}

type JWTClaims struct {
	Sub string `json:"sub"` // user ID
	Exp int64  `json:"exp"` // unix timestamp
	Iat int64  `json:"iat"`
}

// IssueJWT creates a signed JWT for a user session.
func IssueJWT(userID, secret string, ttl time.Duration) (string, error) {
	hdr, _ := json.Marshal(jwtHeader{Alg: "HS256", Typ: "JWT"})
	now := time.Now().UTC()
	claims, _ := json.Marshal(JWTClaims{
		Sub: userID,
		Iat: now.Unix(),
		Exp: now.Add(ttl).Unix(),
	})
	h := base64.RawURLEncoding.EncodeToString(hdr)
	p := base64.RawURLEncoding.EncodeToString(claims)
	sig := jwtSign(h+"."+p, secret)
	return h + "." + p + "." + sig, nil
}

// VerifyJWT validates a JWT and returns claims, or an error.
func VerifyJWT(token, secret string) (*JWTClaims, error) {
	parts := strings.Split(token, ".")
	if len(parts) != 3 {
		return nil, fmt.Errorf("malformed token")
	}
	expectedSig := jwtSign(parts[0]+"."+parts[1], secret)
	if !hmac.Equal([]byte(parts[2]), []byte(expectedSig)) {
		return nil, fmt.Errorf("invalid signature")
	}
	claimBytes, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return nil, err
	}
	var claims JWTClaims
	if err := json.Unmarshal(claimBytes, &claims); err != nil {
		return nil, err
	}
	if time.Now().Unix() > claims.Exp {
		return nil, fmt.Errorf("token expired")
	}
	return &claims, nil
}

func jwtSign(payload, secret string) string {
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write([]byte(payload))
	return base64.RawURLEncoding.EncodeToString(mac.Sum(nil))
}
