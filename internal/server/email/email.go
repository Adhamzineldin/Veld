// Package email sends transactional emails via SMTP.
package email

import (
	"fmt"
	"net/smtp"
	"strings"
)

// Config holds SMTP connection settings.
type Config struct {
	Host     string `json:"host"`
	Port     int    `json:"port"`
	Username string `json:"username"`
	Password string `json:"password"`
	From     string `json:"from"` // e.g. "Veld Registry <noreply@registry.veld.dev>"
}

// Enabled returns true when SMTP is configured.
func (c Config) Enabled() bool { return c.Host != "" && c.From != "" }

// Send delivers an HTML email. Returns nil if SMTP is not configured (silent skip).
func Send(cfg Config, to, subject, htmlBody string) error {
	if !cfg.Enabled() {
		return nil // email disabled — skip silently
	}
	port := cfg.Port
	if port == 0 {
		port = 587
	}
	addr := fmt.Sprintf("%s:%d", cfg.Host, port)

	var auth smtp.Auth
	if cfg.Username != "" {
		auth = smtp.PlainAuth("", cfg.Username, cfg.Password, cfg.Host)
	}

	msg := strings.Join([]string{
		"From: " + cfg.From,
		"To: " + to,
		"Subject: " + subject,
		"MIME-Version: 1.0",
		"Content-Type: text/html; charset=UTF-8",
		"",
		htmlBody,
	}, "\r\n")

	fromAddr := cfg.From
	if i := strings.Index(fromAddr, "<"); i >= 0 {
		fromAddr = strings.Trim(fromAddr[i:], "<> ")
	}

	return smtp.SendMail(addr, auth, fromAddr, []string{to}, []byte(msg))
}

// ── Email templates ────────────────────────────────────────────────────────────

func VerificationEmail(username, verifyURL string) string {
	return fmt.Sprintf(`<!DOCTYPE html>
<html>
<body style="font-family:system-ui,sans-serif;background:#0d1117;color:#e6edf3;padding:40px 20px;margin:0">
  <div style="max-width:480px;margin:0 auto;background:#161b22;border:1px solid #30363d;border-radius:12px;padding:32px">
    <div style="font-size:24px;font-weight:800;color:#6366f1;margin-bottom:8px">⬡ Veld Registry</div>
    <h2 style="margin:0 0 16px;font-size:18px">Verify your email address</h2>
    <p style="color:#8b949e;margin:0 0 24px">Hi %s, click the button below to verify your email and activate your account.</p>
    <a href="%s" style="display:inline-block;background:#6366f1;color:#fff;padding:12px 28px;border-radius:8px;text-decoration:none;font-weight:600">Verify Email</a>
    <p style="color:#8b949e;font-size:12px;margin:24px 0 0">This link expires in 24 hours. If you didn't create an account, ignore this email.</p>
  </div>
</body>
</html>`, username, verifyURL)
}

func WelcomeEmail(username string) string {
	return fmt.Sprintf(`<!DOCTYPE html>
<html>
<body style="font-family:system-ui,sans-serif;background:#0d1117;color:#e6edf3;padding:40px 20px;margin:0">
  <div style="max-width:480px;margin:0 auto;background:#161b22;border:1px solid #30363d;border-radius:12px;padding:32px">
    <div style="font-size:24px;font-weight:800;color:#6366f1;margin-bottom:8px">⬡ Veld Registry</div>
    <h2 style="margin:0 0 16px;font-size:18px">Welcome, %s!</h2>
    <p style="color:#8b949e;margin:0 0 16px">Your email has been verified. You can now create organisations, publish packages, and collaborate with your team.</p>
    <p style="color:#8b949e;margin:0">Start by creating an organisation and publishing your first contract.</p>
  </div>
</body>
</html>`, username)
}
