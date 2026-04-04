package strategy

import "testing"

func TestActionRelativeRouteTemplate(t *testing.T) {
	tests := []struct {
		modulePrefix, actionPath, want string
	}{
		{"/api/auth", "/login", "login"},
		{"/api/auth", "/register", "register"},
		{"/api/users", "/", ""},
		{"/api/users", "/:id", "{id}"},
		{"", "/items/:id", "items/{id}"},
		{"/api", "/auth/login", "auth/login"},
		// Defensive: action path repeats module route prefix
		{"/api/auth", "/api/auth/login", "login"},
		{"/api/auth", "/api/auth", ""},
	}
	for _, tt := range tests {
		got := actionRelativeRouteTemplate(tt.modulePrefix, tt.actionPath)
		if got != tt.want {
			t.Errorf("actionRelativeRouteTemplate(%q, %q) = %q, want %q", tt.modulePrefix, tt.actionPath, got, tt.want)
		}
	}
}

func TestAspNetStrategy_RouteAnnotation_Relative(t *testing.T) {
	s := &AspNetStrategy{}
	if g := s.RouteAnnotation("POST", "/login", "/api/auth"); g != `[HttpPost("login")]` {
		t.Errorf("POST /login under /api/auth: got %s", g)
	}
	if g := s.RouteAnnotation("GET", "/", "/api/users"); g != "[HttpGet]" {
		t.Errorf("GET / under /api/users: got %s, want [HttpGet]", g)
	}
	if g := s.RouteAnnotation("GET", "/:id", "/api/users"); g != `[HttpGet("{id}")]` {
		t.Errorf("GET /:id: got %s", g)
	}
}
