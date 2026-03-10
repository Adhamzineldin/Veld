package auth

import (
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha1"
	"encoding/base32"
	"encoding/binary"
	"fmt"
	"math"
	"net/url"
	"time"
)

// GenerateTOTPSecret creates a new random base32-encoded TOTP secret (160-bit).
func GenerateTOTPSecret() (string, error) {
	b := make([]byte, 20)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base32.StdEncoding.WithPadding(base32.NoPadding).EncodeToString(b), nil
}

// VerifyTOTP checks a 6-digit TOTP code with a ±1 step (90-second) window.
func VerifyTOTP(secret, code string) bool {
	now := time.Now().Unix()
	for _, offset := range []int64{-1, 0, 1} {
		c, err := totpAt(secret, now+offset*30)
		if err == nil && c == code {
			return true
		}
	}
	return false
}

// CurrentTOTP returns the current 6-digit TOTP code (for testing).
func CurrentTOTP(secret string) (string, error) {
	return totpAt(secret, time.Now().Unix())
}

func totpAt(secret string, t int64) (string, error) {
	key, err := base32.StdEncoding.WithPadding(base32.NoPadding).DecodeString(secret)
	if err != nil {
		return "", err
	}
	counter := uint64(math.Floor(float64(t) / 30))
	buf := make([]byte, 8)
	binary.BigEndian.PutUint64(buf, counter)
	mac := hmac.New(sha1.New, key)
	mac.Write(buf)
	h := mac.Sum(nil)
	offset := h[len(h)-1] & 0x0f
	code := binary.BigEndian.Uint32(h[offset:offset+4]) & 0x7fffffff
	return fmt.Sprintf("%06d", code%1_000_000), nil
}

// OTPAuthURI returns the otpauth:// URI used to generate QR codes.
func OTPAuthURI(secret, username, issuer string) string {
	return fmt.Sprintf(
		"otpauth://totp/%s:%s?secret=%s&issuer=%s&algorithm=SHA1&digits=6&period=30",
		url.PathEscape(issuer),
		url.PathEscape(username),
		secret,
		url.QueryEscape(issuer),
	)
}
