package sdkhelpers

import "testing"

func TestEnvVarName(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"iam", "VELD_IAM_URL"},
		{"card-service", "VELD_CARD_SERVICE_URL"},
		{"transactions", "VELD_TRANSACTIONS_URL"},
		{"my service", "VELD_MY_SERVICE_URL"},
	}
	for _, tt := range tests {
		got := EnvVarName(tt.input)
		if got != tt.want {
			t.Errorf("EnvVarName(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestServiceClassName(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"iam", "IamClient"},
		{"card-service", "CardServiceClient"},
		{"my_thing", "MyThingClient"},
	}
	for _, tt := range tests {
		got := ServiceClassName(tt.input)
		if got != tt.want {
			t.Errorf("ServiceClassName(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestServiceFileName(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"iam", "iam"},
		{"card-service", "card_service"},
		{"IAM", "iam"},
	}
	for _, tt := range tests {
		got := ServiceFileName(tt.input)
		if got != tt.want {
			t.Errorf("ServiceFileName(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}
