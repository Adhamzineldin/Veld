package emitter

import "testing"

func TestToEncodedTemplateLiteral(t *testing.T) {
	cases := []struct{ path, want string }{
		{"/items", "/items"},
		{"/items/:id", "/items/${encodeURIComponent(id)}"},
		{"/orgs/:orgId/members/:userId", "/orgs/${encodeURIComponent(orgId)}/members/${encodeURIComponent(userId)}"},
	}
	for _, tc := range cases {
		if got := ToEncodedTemplateLiteral(tc.path); got != tc.want {
			t.Errorf("ToEncodedTemplateLiteral(%q) = %q, want %q", tc.path, got, tc.want)
		}
	}
}
