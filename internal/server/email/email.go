// Package email sends transactional emails via SMTP.
package email

import (
	"crypto/tls"
	"fmt"
	"log"
	"net"
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

// loginAuth implements the LOGIN auth mechanism required by Office 365 / Exchange Online.
// Go's standard smtp.PlainAuth only supports PLAIN, which Office 365 rejects with
// "504 5.7.4 Unrecognized authentication type".
type loginAuth struct {
	username, password string
}

func (a *loginAuth) Start(server *smtp.ServerInfo) (string, []byte, error) {
	return "LOGIN", []byte(a.username), nil
}

func (a *loginAuth) Next(fromServer []byte, more bool) ([]byte, error) {
	if more {
		prompt := strings.TrimSpace(string(fromServer))
		switch strings.ToLower(prompt) {
		case "username:":
			return []byte(a.username), nil
		case "password:":
			return []byte(a.password), nil
		default:
			return nil, fmt.Errorf("unexpected LOGIN prompt: %q", prompt)
		}
	}
	return nil, nil
}

// Send delivers an HTML email. Returns nil if SMTP is not configured (silent skip).
func Send(cfg Config, to, subject, htmlBody string) error {
	if !cfg.Enabled() {
		log.Printf("[smtp] Send skipped — SMTP not configured (host=%q from=%q)", cfg.Host, cfg.From)
		return nil
	}
	port := cfg.Port
	if port == 0 {
		port = 587
	}
	addr := fmt.Sprintf("%s:%d", cfg.Host, port)
	log.Printf("[smtp] connecting to %s (from=%s, to=%s, subject=%q)", addr, cfg.From, to, subject)

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

	// Connect to the SMTP server
	conn, err := net.DialTimeout("tcp", addr, 30_000_000_000) // 30s
	if err != nil {
		log.Printf("[smtp] ERROR dial %s: %v", addr, err)
		return fmt.Errorf("smtp dial: %w", err)
	}

	client, err := smtp.NewClient(conn, cfg.Host)
	if err != nil {
		conn.Close()
		log.Printf("[smtp] ERROR creating SMTP client: %v", err)
		return fmt.Errorf("smtp client: %w", err)
	}
	defer client.Close()

	// STARTTLS — required by Office 365 on port 587
	if ok, _ := client.Extension("STARTTLS"); ok {
		log.Printf("[smtp] STARTTLS supported — upgrading connection")
		tlsCfg := &tls.Config{ServerName: cfg.Host}
		if err := client.StartTLS(tlsCfg); err != nil {
			log.Printf("[smtp] ERROR STARTTLS: %v", err)
			return fmt.Errorf("smtp starttls: %w", err)
		}
	} else {
		log.Printf("[smtp] STARTTLS not advertised — continuing without TLS")
	}

	// Authenticate — try LOGIN first (Office 365), fall back to PLAIN
	if cfg.Username != "" {
		var authErr error

		// Check which mechanisms the server advertises
		authMechs := ""
		if ok, mechs := client.Extension("AUTH"); ok {
			authMechs = mechs
		}
		log.Printf("[smtp] server AUTH mechanisms: %q", authMechs)

		if strings.Contains(strings.ToUpper(authMechs), "LOGIN") {
			log.Printf("[smtp] using LOGIN auth with username=%s", cfg.Username)
			authErr = client.Auth(&loginAuth{cfg.Username, cfg.Password})
		} else if strings.Contains(strings.ToUpper(authMechs), "PLAIN") {
			log.Printf("[smtp] using PLAIN auth with username=%s", cfg.Username)
			authErr = client.Auth(smtp.PlainAuth("", cfg.Username, cfg.Password, cfg.Host))
		} else {
			// Server didn't advertise known mechanisms — try LOGIN anyway
			log.Printf("[smtp] no recognized AUTH mechanism in %q — trying LOGIN", authMechs)
			authErr = client.Auth(&loginAuth{cfg.Username, cfg.Password})
		}

		if authErr != nil {
			log.Printf("[smtp] ERROR auth: %v", authErr)
			return fmt.Errorf("smtp auth: %w", authErr)
		}
		log.Printf("[smtp] authenticated successfully")
	} else {
		log.Printf("[smtp] no auth credentials configured — skipping auth")
	}

	// Set sender and recipient
	if err := client.Mail(fromAddr); err != nil {
		log.Printf("[smtp] ERROR MAIL FROM: %v", err)
		return fmt.Errorf("smtp mail from: %w", err)
	}
	if err := client.Rcpt(to); err != nil {
		log.Printf("[smtp] ERROR RCPT TO: %v", err)
		return fmt.Errorf("smtp rcpt to: %w", err)
	}

	// Write the message body
	w, err := client.Data()
	if err != nil {
		log.Printf("[smtp] ERROR DATA: %v", err)
		return fmt.Errorf("smtp data: %w", err)
	}
	if _, err := w.Write([]byte(msg)); err != nil {
		log.Printf("[smtp] ERROR writing body: %v", err)
		return fmt.Errorf("smtp write: %w", err)
	}
	if err := w.Close(); err != nil {
		log.Printf("[smtp] ERROR closing body: %v", err)
		return fmt.Errorf("smtp close: %w", err)
	}

	if err := client.Quit(); err != nil {
		// Quit error is non-fatal — message was already accepted
		log.Printf("[smtp] QUIT warning: %v", err)
	}

	log.Printf("[smtp] mail delivered successfully to %s", to)
	return nil
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
