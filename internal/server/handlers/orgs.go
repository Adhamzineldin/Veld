package handlers

import (
	"encoding/json"
	"net/http"
	"time"

	serverauth "github.com/Adhamzineldin/Veld/internal/server/auth"
	"github.com/Adhamzineldin/Veld/internal/server/db"
	"github.com/Adhamzineldin/Veld/internal/server/models"
)

// OrgHandler handles organization endpoints.
type OrgHandler struct{ DB *db.DB }

type createOrgBody struct {
	Name        string `json:"name"`
	DisplayName string `json:"display_name"`
	Description string `json:"description"`
}

// CreateOrg handles POST /api/v1/orgs
func (h *OrgHandler) CreateOrg(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	var body createOrgBody
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil || body.Name == "" {
		jsonError(w, "name is required", http.StatusBadRequest)
		return
	}

	// Validate name: lowercase letters, digits, hyphens only
	for _, c := range body.Name {
		if !((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-') {
			jsonError(w, "org name must be lowercase letters, digits and hyphens only", http.StatusBadRequest)
			return
		}
	}

	org := &models.Org{
		ID:          serverauth.GenerateID(),
		Name:        body.Name,
		DisplayName: body.DisplayName,
		Description: body.Description,
		CreatedAt:   time.Now().UTC(),
	}
	if err := h.DB.CreateOrg(org); err != nil {
		jsonError(w, "org name already taken", http.StatusConflict)
		return
	}
	// Creator becomes owner
	h.DB.AddOrgMember(org.ID, u.ID, "owner")
	jsonCreated(w, org)
}

// ListOrgs handles GET /api/v1/orgs
func (h *OrgHandler) ListOrgs(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	var orgs []*models.Org
	var err error
	if u != nil {
		orgs, err = h.DB.ListOrgsForUser(u.ID)
	} else {
		orgs, err = h.DB.ListOrgs()
	}
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	if orgs == nil {
		orgs = []*models.Org{}
	}
	jsonOK(w, orgs)
}

// GetOrg handles GET /api/v1/orgs/{org}
func (h *OrgHandler) GetOrg(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("org")
	org, err := h.DB.GetOrgByName(name)
	if err != nil || org == nil {
		jsonError(w, "org not found", http.StatusNotFound)
		return
	}
	members, _ := h.DB.ListOrgMembers(org.ID)
	packages, _ := h.DB.ListPackagesForOrg(org.ID)
	if members == nil {
		members = []*models.OrgMember{}
	}
	if packages == nil {
		packages = []*models.Package{}
	}
	jsonOK(w, map[string]interface{}{
		"org":      org,
		"members":  members,
		"packages": packages,
	})
}

type addMemberBody struct {
	Username string `json:"username"`
	Role     string `json:"role"`
}

// AddMember handles POST /api/v1/orgs/{org}/members
func (h *OrgHandler) AddMember(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	org, err := h.DB.GetOrgByName(r.PathValue("org"))
	if err != nil || org == nil {
		jsonError(w, "org not found", http.StatusNotFound)
		return
	}
	caller, err := h.DB.GetOrgMember(org.ID, u.ID)
	if err != nil || caller == nil || (caller.Role != "owner" && caller.Role != "admin") {
		jsonError(w, "forbidden", http.StatusForbidden)
		return
	}

	var body addMemberBody
	json.NewDecoder(r.Body).Decode(&body)
	if body.Username == "" {
		jsonError(w, "username is required", http.StatusBadRequest)
		return
	}
	if body.Role == "" {
		body.Role = "member"
	}
	if body.Role != "owner" && body.Role != "admin" && body.Role != "member" {
		jsonError(w, "role must be owner, admin or member", http.StatusBadRequest)
		return
	}
	// Non-owners cannot grant owner role
	if body.Role == "owner" && caller.Role != "owner" {
		jsonError(w, "only owners can grant owner role", http.StatusForbidden)
		return
	}

	target, err := h.DB.GetUserByUsername(body.Username)
	if err != nil || target == nil {
		jsonError(w, "user not found", http.StatusNotFound)
		return
	}
	if err := h.DB.AddOrgMember(org.ID, target.ID, body.Role); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	jsonOK(w, map[string]string{"message": "member added"})
}

// RemoveMember handles DELETE /api/v1/orgs/{org}/members/{username}
func (h *OrgHandler) RemoveMember(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	org, err := h.DB.GetOrgByName(r.PathValue("org"))
	if err != nil || org == nil {
		jsonError(w, "org not found", http.StatusNotFound)
		return
	}
	caller, err := h.DB.GetOrgMember(org.ID, u.ID)
	if err != nil || caller == nil || (caller.Role != "owner" && caller.Role != "admin") {
		jsonError(w, "forbidden", http.StatusForbidden)
		return
	}

	target, err := h.DB.GetUserByUsername(r.PathValue("username"))
	if err != nil || target == nil {
		jsonError(w, "user not found", http.StatusNotFound)
		return
	}
	// Prevent removing last owner
	if caller.Role == "owner" {
		tgtMember, _ := h.DB.GetOrgMember(org.ID, target.ID)
		if tgtMember != nil && tgtMember.Role == "owner" {
			count, _ := h.DB.CountOrgOwners(org.ID)
			if count <= 1 {
				jsonError(w, "cannot remove the last owner", http.StatusConflict)
				return
			}
		}
	}

	h.DB.RemoveOrgMember(org.ID, target.ID)
	jsonNoContent(w)
}
