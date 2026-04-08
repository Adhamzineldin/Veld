package docsgen

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// BuildHTML generates a standalone single-file HTML SPA for API documentation.
// services carries workspace grouping; pass nil for single-service mode.
func BuildHTML(a ast.AST, services []ServiceInfo) string {
	if len(services) == 0 {
		names := make([]string, len(a.Modules))
		for i, m := range a.Modules {
			names[i] = m.Name
		}
		services = []ServiceInfo{{Name: "API", ModuleNames: names}}
	}

	modelSet := make(map[string]bool, len(a.Models))
	for _, m := range a.Models {
		modelSet[m.Name] = true
	}
	enumSet := make(map[string]bool, len(a.Enums))
	for _, e := range a.Enums {
		enumSet[e.Name] = true
	}
	moduleByName := make(map[string]ast.Module, len(a.Modules))
	for _, mod := range a.Modules {
		moduleByName[mod.Name] = mod
	}

	typeLink := func(t string) string {
		if t == "" {
			return ""
		}
		isArray := strings.HasSuffix(t, "[]")
		base := strings.TrimSuffix(t, "[]")
		suffix := ""
		if isArray {
			suffix = "[]"
		}
		if modelSet[base] {
			return fmt.Sprintf(`<a href="#model-%s" class="type-link" onclick="navigateTo('model-%s')">%s</a>%s`, slug(base), slug(base), esc(base), suffix)
		}
		if enumSet[base] {
			return fmt.Sprintf(`<a href="#enum-%s" class="type-link" onclick="navigateTo('enum-%s')">%s</a>%s`, slug(base), slug(base), esc(base), suffix)
		}
		return "<code>" + esc(t) + "</code>"
	}

	fieldTypeHTML := func(f ast.Field) string {
		if f.IsMap {
			return "Map&lt;string, " + typeLink(f.MapValueType) + "&gt;"
		}
		t := f.Type
		if f.IsArray {
			t += "[]"
		}
		return typeLink(t)
	}

	var sb strings.Builder
	sb.WriteString(htmlHead)
	sb.WriteString(`<div class="toolbar"><button class="theme-btn" onclick="toggleTheme()" title="Toggle dark mode">🌙</button></div>` + "\n")

	// ── Sidebar ──
	sb.WriteString(`<nav class="sidebar">` + "\n")
	sb.WriteString(`  <div class="sidebar-header"><h1><span>Veld</span> API</h1><p>Auto-generated documentation</p></div>` + "\n")
	sb.WriteString(`  <div class="search-box"><input type="text" id="search" placeholder="Search…" oninput="filterNav(this.value)"></div>` + "\n")
	sb.WriteString(`  <div class="nav-scroll">` + "\n")

	for _, svc := range services {
		svcSlug := slug(svc.Name)
		actionCount := 0
		for _, mn := range svc.ModuleNames {
			if mod, ok := moduleByName[mn]; ok {
				actionCount += len(mod.Actions)
			}
		}
		sb.WriteString(fmt.Sprintf(`    <details class="tree-svc" open data-search="%s">`, strings.ToLower(svc.Name)))
		sb.WriteString(fmt.Sprintf(`<summary class="tree-svc-title"><span>%s</span><span class="badge">%d</span></summary>`+"\n", esc(svc.Name), actionCount))

		for _, mn := range svc.ModuleNames {
			mod, ok := moduleByName[mn]
			if !ok {
				continue
			}
			modSlug := slug(mn)
			sb.WriteString(fmt.Sprintf(`      <details class="tree-mod" open data-search="%s">`, strings.ToLower(mn)))
			sb.WriteString(fmt.Sprintf(`<summary class="tree-mod-title">%s <span class="badge">%d</span></summary>`+"\n", esc(mod.Name), len(mod.Actions)))

			for _, act := range mod.Actions {
				actID := fmt.Sprintf("action-%s-%s-%s", svcSlug, modSlug, slug(act.Name))
				dotColor := methodColor(act.Method)
				sb.WriteString(fmt.Sprintf(`        <a href="#%s" class="nav-link" data-search="%s %s %s" onclick="navigateTo('%s')"><span class="method-dot" style="background:%s"></span>%s</a>`+"\n",
					actID, strings.ToLower(act.Name), strings.ToLower(act.Method), strings.ToLower(act.Path), actID, dotColor, esc(act.Name)))
			}
			sb.WriteString("      </details>\n")
		}
		sb.WriteString("    </details>\n")
	}

	if len(a.Models) > 0 {
		sb.WriteString(`    <details class="tree-section" open><summary class="tree-section-title">Models <span class="badge">` + fmt.Sprintf("%d", len(a.Models)) + `</span></summary>` + "\n")
		for _, m := range a.Models {
			sb.WriteString(fmt.Sprintf(`      <a href="#model-%s" class="nav-link" data-search="%s" onclick="navigateTo('model-%s')">%s</a>`+"\n", slug(m.Name), strings.ToLower(m.Name), slug(m.Name), esc(m.Name)))
		}
		sb.WriteString("    </details>\n")
	}
	if len(a.Enums) > 0 {
		sb.WriteString(`    <details class="tree-section" open><summary class="tree-section-title">Enums <span class="badge">` + fmt.Sprintf("%d", len(a.Enums)) + `</span></summary>` + "\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf(`      <a href="#enum-%s" class="nav-link" data-search="%s" onclick="navigateTo('enum-%s')">%s</a>`+"\n", slug(en.Name), strings.ToLower(en.Name), slug(en.Name), esc(en.Name)))
		}
		sb.WriteString("    </details>\n")
	}
	sb.WriteString("  </div>\n</nav>\n")

	// ── Main content ──
	sb.WriteString(`<main class="main">` + "\n")
	sb.WriteString(`<div class="breadcrumbs" id="breadcrumbs"><span class="bc-item">API Documentation</span></div>` + "\n")

	totalActions := 0
	for _, mod := range a.Modules {
		totalActions += len(mod.Actions)
	}
	sb.WriteString(`<div class="stats">` + "\n")
	if len(services) > 1 {
		sb.WriteString(fmt.Sprintf(`  <div class="stat"><div class="stat-value">%d</div><div class="stat-label">Services</div></div>`+"\n", len(services)))
	}
	sb.WriteString(fmt.Sprintf(`  <div class="stat"><div class="stat-value">%d</div><div class="stat-label">Modules</div></div>`+"\n", len(a.Modules)))
	sb.WriteString(fmt.Sprintf(`  <div class="stat"><div class="stat-value">%d</div><div class="stat-label">Endpoints</div></div>`+"\n", totalActions))
	sb.WriteString(fmt.Sprintf(`  <div class="stat"><div class="stat-value">%d</div><div class="stat-label">Models</div></div>`+"\n", len(a.Models)))
	sb.WriteString(fmt.Sprintf(`  <div class="stat"><div class="stat-value">%d</div><div class="stat-label">Enums</div></div>`+"\n", len(a.Enums)))
	sb.WriteString("</div>\n\n")

	// ── Per-service sections ──
	for _, svc := range services {
		svcSlug := slug(svc.Name)
		multiService := len(services) > 1

		if multiService {
			sb.WriteString(fmt.Sprintf(`<section id="service-%s" class="service-section" data-bc="%s">`, svcSlug, esc(svc.Name)))
			sb.WriteString(fmt.Sprintf(`<h2 class="service-title">%s`, esc(svc.Name)))
			if svc.BaseUrl != "" {
				sb.WriteString(fmt.Sprintf(` <code class="base-url">%s</code>`, esc(svc.BaseUrl)))
			}
			sb.WriteString("</h2>\n")
			if svc.Description != "" {
				sb.WriteString(fmt.Sprintf(`<p class="section-desc">%s</p>`+"\n", esc(svc.Description)))
			}
			svcActions := 0
			for _, mn := range svc.ModuleNames {
				if mod, ok := moduleByName[mn]; ok {
					svcActions += len(mod.Actions)
				}
			}
			usedModels := make(map[string]bool)
			for _, mn := range svc.ModuleNames {
				if mod, ok := moduleByName[mn]; ok {
					for _, act := range mod.Actions {
						if act.Input != "" {
							usedModels[act.Input] = true
						}
						if act.Output != "" {
							usedModels[act.Output] = true
						}
					}
				}
			}
			sb.WriteString(fmt.Sprintf(`<div class="service-stats"><span>%d endpoints</span><span>%d modules</span><span>%d models</span></div>`+"\n", svcActions, len(svc.ModuleNames), len(usedModels)))
		}

		for _, mn := range svc.ModuleNames {
			mod, ok := moduleByName[mn]
			if !ok {
				continue
			}
			modSlug := slug(mn)
			modBC := esc(svc.Name) + " › " + esc(mod.Name)

			sb.WriteString(fmt.Sprintf(`<h3 id="mod-%s-%s" class="module-title" data-bc="%s">%s`, svcSlug, modSlug, modBC, esc(mod.Name)))
			if mod.Prefix != "" {
				sb.WriteString(fmt.Sprintf(` <code class="prefix-badge">%s</code>`, esc(mod.Prefix)))
			}
			sb.WriteString("</h3>\n")
			if mod.Description != "" {
				sb.WriteString(fmt.Sprintf(`<p class="section-desc">%s</p>`+"\n", esc(mod.Description)))
			}

			for _, act := range mod.Actions {
				actID := fmt.Sprintf("action-%s-%s-%s", svcSlug, modSlug, slug(act.Name))
				routePath := act.Path
				if mod.Prefix != "" {
					routePath = mod.Prefix + act.Path
				}
				method := strings.ToUpper(act.Method)
				actBC := modBC + " › " + esc(act.Name)

				highlightedPath := esc(routePath)
				for _, seg := range strings.Split(routePath, "/") {
					if strings.HasPrefix(seg, ":") {
						highlightedPath = strings.Replace(highlightedPath, seg, `<span class="param">`+seg+`</span>`, 1)
					}
				}

				depCls := ""
				if act.Deprecated != "" {
					depCls = " deprecated"
				}
				sb.WriteString(fmt.Sprintf(`<div class="endpoint%s" id="%s" data-bc="%s">`+"\n", depCls, actID, actBC))
				sb.WriteString(`  <div class="endpoint-header" onclick="toggleDetail(this)">` + "\n")
				sb.WriteString(fmt.Sprintf(`    <span class="method-badge method-%s">%s</span>`, method, method))
				sb.WriteString(fmt.Sprintf(`    <span class="endpoint-path">%s</span>`, highlightedPath))
				sb.WriteString(fmt.Sprintf(`    <span class="endpoint-name">%s`, esc(act.Name)))
				if act.Deprecated != "" {
					sb.WriteString(fmt.Sprintf(` <span class="deprecated-badge" title="%s">deprecated</span>`, esc(act.Deprecated)))
				}
				sb.WriteString(`</span>`)
				sb.WriteString("\n    <span class=\"chevron\">▸</span>\n  </div>\n")
				sb.WriteString(`  <div class="endpoint-detail">` + "\n")
				if act.Description != "" {
					sb.WriteString(fmt.Sprintf(`    <div class="detail-row"><span class="detail-label">Description</span><span>%s</span></div>`+"\n", esc(act.Description)))
				}
				if act.Input != "" {
					sb.WriteString(fmt.Sprintf(`    <div class="detail-row"><span class="detail-label">Input</span><span class="detail-value">%s</span></div>`+"\n", typeLink(act.Input)))
				}
				output := act.Output
				if act.OutputArray {
					output += "[]"
				}
				if output != "" {
					sb.WriteString(fmt.Sprintf(`    <div class="detail-row"><span class="detail-label">Output</span><span class="detail-value">%s</span></div>`+"\n", typeLink(output)))
				} else {
					sb.WriteString(`    <div class="detail-row"><span class="detail-label">Output</span><span class="detail-value"><code>void</code></span></div>` + "\n")
				}
				if len(act.OutputFields) > 0 {
					sb.WriteString(`    <div class="inline-fields"><div class="inline-fields-title">Output Fields</div><table class="model-table"><thead><tr><th>Field</th><th>Type</th><th>Attr</th></tr></thead><tbody>` + "\n")
					for _, f := range act.OutputFields {
						sb.WriteString(fmt.Sprintf(`      <tr><td><strong>%s</strong></td><td>%s</td><td>%s</td></tr>`+"\n", esc(f.Name), fieldTypeHTML(f), fieldAttrs(f)))
					}
					sb.WriteString("    </tbody></table></div>\n")
				}
				if act.Query != "" {
					sb.WriteString(fmt.Sprintf(`    <div class="detail-row"><span class="detail-label">Query</span><span class="detail-value">%s</span></div>`+"\n", typeLink(act.Query)))
				}
				if len(act.QueryFields) > 0 {
					sb.WriteString(`    <div class="inline-fields"><div class="inline-fields-title">Query Fields</div><table class="model-table"><thead><tr><th>Field</th><th>Type</th><th>Attr</th></tr></thead><tbody>` + "\n")
					for _, f := range act.QueryFields {
						sb.WriteString(fmt.Sprintf(`      <tr><td><strong>%s</strong></td><td>%s</td><td>%s</td></tr>`+"\n", esc(f.Name), fieldTypeHTML(f), fieldAttrs(f)))
					}
					sb.WriteString("    </tbody></table></div>\n")
				}
				if len(act.Middleware) > 0 {
					sb.WriteString(fmt.Sprintf(`    <div class="detail-row"><span class="detail-label">Middleware</span><span class="detail-value"><code>%s</code></span></div>`+"\n", esc(strings.Join(act.Middleware, ", "))))
				}
				if len(act.Errors) > 0 {
					sb.WriteString(`    <div class="detail-row"><span class="detail-label">Errors</span><span class="detail-value">`)
					for i, e := range act.Errors {
						if i > 0 {
							sb.WriteString(", ")
						}
						if status, ok := act.ErrorStatuses[e]; ok {
							sb.WriteString(fmt.Sprintf(`<code>%s (%d)</code>`, esc(e), status))
						} else {
							sb.WriteString(fmt.Sprintf(`<code>%s</code>`, esc(e)))
						}
					}
					sb.WriteString("</span></div>\n")
				}
				sb.WriteString("  </div>\n</div>\n")
			}
		}

		if multiService {
			sb.WriteString("</section>\n\n")
		}
	}

	// ── Models ──
	if len(a.Models) > 0 {
		sb.WriteString(`<h2 class="section-divider" data-bc="Models">Models</h2>` + "\n")
		for _, m := range a.Models {
			sb.WriteString(fmt.Sprintf(`<div class="model-card" id="model-%s" data-bc="Models › %s">`+"\n", slug(m.Name), esc(m.Name)))
			sb.WriteString(fmt.Sprintf(`  <div class="model-header"><h3>%s`, esc(m.Name)))
			if m.Extends != "" {
				sb.WriteString(fmt.Sprintf(` <span class="extends">extends %s</span>`, typeLink(m.Extends)))
			}
			sb.WriteString("</h3>\n")
			if m.Description != "" {
				sb.WriteString(fmt.Sprintf(`    <div class="model-desc">%s</div>`+"\n", esc(m.Description)))
			}
			usedBy := collectModelUsages(a, m.Name)
			if len(usedBy) > 0 {
				sb.WriteString(`    <div class="used-by">Used by: `)
				for i, u := range usedBy {
					if i > 0 {
						sb.WriteString(", ")
					}
					sb.WriteString(fmt.Sprintf(`<span class="used-by-action">%s</span>`, esc(u)))
				}
				sb.WriteString("</div>\n")
			}
			sb.WriteString("  </div>\n")
			if len(m.Fields) > 0 {
				sb.WriteString("  <table class=\"model-table\"><thead><tr><th>Field</th><th>Type</th><th>Attributes</th></tr></thead><tbody>\n")
				for _, f := range m.Fields {
					depCls := ""
					if f.Deprecated != "" {
						depCls = ` class="field-deprecated"`
					}
					sb.WriteString(fmt.Sprintf(`    <tr%s><td><strong>%s</strong>`, depCls, esc(f.Name)))
					if f.Deprecated != "" {
						sb.WriteString(fmt.Sprintf(` <span class="deprecated-badge" title="%s">deprecated</span>`, esc(f.Deprecated)))
					}
					sb.WriteString(fmt.Sprintf(`</td><td>%s</td><td>%s</td></tr>`+"\n", fieldTypeHTML(f), fieldAttrs(f)))
				}
				sb.WriteString("  </tbody></table>\n")
			}
			sb.WriteString("</div>\n")
		}
	}

	// ── Enums ──
	if len(a.Enums) > 0 {
		sb.WriteString(`<h2 class="section-divider" data-bc="Enums">Enums</h2>` + "\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf(`<div class="model-card" id="enum-%s" data-bc="Enums › %s">`+"\n", slug(en.Name), esc(en.Name)))
			sb.WriteString(fmt.Sprintf(`  <div class="model-header"><h3>%s</h3>`, esc(en.Name)))
			if en.Description != "" {
				sb.WriteString(fmt.Sprintf(`<div class="model-desc">%s</div>`, esc(en.Description)))
			}
			usedBy := collectEnumUsages(a, en.Name)
			if len(usedBy) > 0 {
				sb.WriteString(`<div class="used-by">Used by: `)
				for i, u := range usedBy {
					if i > 0 {
						sb.WriteString(", ")
					}
					sb.WriteString(fmt.Sprintf(`<span class="used-by-action">%s</span>`, esc(u)))
				}
				sb.WriteString("</div>")
			}
			sb.WriteString("</div>\n  <div class=\"enum-values\">\n")
			for _, v := range en.Values {
				sb.WriteString(fmt.Sprintf(`    <span class="enum-value">%s</span>`+"\n", esc(v)))
			}
			sb.WriteString("  </div>\n</div>\n")
		}
	}

	sb.WriteString(`<div class="footer">Generated by Veld</div>` + "\n")
	sb.WriteString("</main>\n")
	sb.WriteString(htmlScript)
	sb.WriteString("</body>\n</html>\n")
	return sb.String()
}

// ── Helpers ──

func slug(s string) string {
	return strings.ToLower(strings.ReplaceAll(strings.ReplaceAll(s, " ", "-"), "_", "-"))
}

func esc(s string) string {
	r := strings.NewReplacer("&", "&amp;", "<", "&lt;", ">", "&gt;", `"`, "&quot;")
	return r.Replace(s)
}

func fieldAttrs(f ast.Field) string {
	var parts []string
	if f.Optional {
		parts = append(parts, `<span class="optional-badge">optional</span>`)
	}
	if f.Default != "" {
		parts = append(parts, fmt.Sprintf(`<span class="default-value">= %s</span>`, esc(f.Default)))
	}
	if f.Unique {
		parts = append(parts, `<span class="attr-badge">unique</span>`)
	}
	if f.Index {
		parts = append(parts, `<span class="attr-badge">index</span>`)
	}
	if f.Relation != "" {
		parts = append(parts, fmt.Sprintf(`<span class="attr-badge">→ %s</span>`, esc(f.Relation)))
	}
	if f.ServerSet {
		parts = append(parts, `<span class="attr-badge">serverSet</span>`)
	}
	if len(parts) == 0 {
		return "&mdash;"
	}
	return strings.Join(parts, " ")
}

func collectModelUsages(a ast.AST, name string) []string {
	var usages []string
	seen := make(map[string]bool)
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			label := mod.Name + "." + act.Name
			if (act.Input == name || act.Output == name || act.Query == name) && !seen[label] {
				usages = append(usages, label)
				seen[label] = true
			}
		}
	}
	return usages
}

func collectEnumUsages(a ast.AST, name string) []string {
	var usages []string
	seen := make(map[string]bool)
	for _, m := range a.Models {
		for _, f := range m.Fields {
			if (f.Type == name || f.MapValueType == name) && !seen[m.Name] {
				usages = append(usages, m.Name)
				seen[m.Name] = true
			}
		}
	}
	return usages
}

func methodColor(m string) string {
	switch strings.ToUpper(m) {
	case "POST":
		return "var(--post)"
	case "PUT":
		return "var(--put)"
	case "DELETE":
		return "var(--delete)"
	case "PATCH":
		return "var(--patch)"
	case "WS":
		return "var(--ws)"
	default:
		return "var(--get)"
	}
}

// ── CSS + HTML head ─────────────────────────────────────────────────────────

const htmlHead = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>API Documentation — Veld</title>
<style>
*,*::before,*::after{margin:0;padding:0;box-sizing:border-box}
:root{
  --bg:#f9fafb;--fg:#111827;--sidebar-bg:#ffffff;--sidebar-border:#e5e7eb;
  --card:#ffffff;--card-border:#e5e7eb;--card-shadow:0 1px 3px rgba(0,0,0,.06);
  --accent:#4f46e5;--accent-hover:#4338ca;--accent-bg:#eef2ff;--accent-fg:#3730a3;
  --muted:#6b7280;--muted-light:#9ca3af;
  --code-bg:#f3f4f6;--code-fg:#1f2937;
  --table-stripe:#f9fafb;--table-hover:#f3f4f6;
  --get:#059669;--get-bg:#ecfdf5;--post:#2563eb;--post-bg:#eff6ff;
  --put:#d97706;--put-bg:#fffbeb;--delete:#dc2626;--delete-bg:#fef2f2;
  --patch:#ea580c;--patch-bg:#fff7ed;--ws:#7c3aed;--ws-bg:#f5f3ff;
  --radius:8px;--transition:150ms ease;--highlight:#fef08a;
}
[data-theme="dark"]{
  --bg:#0f172a;--fg:#e2e8f0;--sidebar-bg:#1e293b;--sidebar-border:#334155;
  --card:#1e293b;--card-border:#334155;--card-shadow:0 1px 3px rgba(0,0,0,.3);
  --accent:#818cf8;--accent-hover:#a5b4fc;--accent-bg:#1e1b4b;--accent-fg:#c7d2fe;
  --muted:#94a3b8;--muted-light:#64748b;
  --code-bg:#334155;--code-fg:#e2e8f0;
  --table-stripe:#1e293b;--table-hover:#334155;
  --get:#34d399;--get-bg:#064e3b;--post:#60a5fa;--post-bg:#1e3a5f;
  --put:#fbbf24;--put-bg:#451a03;--delete:#f87171;--delete-bg:#450a0a;
  --patch:#fb923c;--patch-bg:#431407;--ws:#a78bfa;--ws-bg:#2e1065;
  --highlight:#854d0e;
}
html{scroll-behavior:smooth}
body{font-family:Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6;display:flex;min-height:100vh}
.sidebar{width:280px;background:var(--sidebar-bg);border-right:1px solid var(--sidebar-border);position:fixed;top:0;left:0;height:100vh;display:flex;flex-direction:column;z-index:20}
.sidebar-header{padding:20px 16px 12px;border-bottom:1px solid var(--sidebar-border)}
.sidebar-header h1{font-size:18px;font-weight:700;letter-spacing:-.02em}
.sidebar-header h1 span{color:var(--accent)}
.sidebar-header p{font-size:11px;color:var(--muted);margin-top:2px}
.search-box{padding:10px 12px;position:relative}
.search-box input{width:100%;padding:7px 10px 7px 32px;border:1px solid var(--card-border);border-radius:var(--radius);font-size:12px;background:var(--bg);color:var(--fg);outline:none;transition:border-color var(--transition)}
.search-box input:focus{border-color:var(--accent)}
.search-box::before{content:"🔍";position:absolute;left:20px;top:50%;transform:translateY(-50%);font-size:12px}
.nav-scroll{flex:1;overflow-y:auto;padding:4px 8px 20px}
.nav-scroll::-webkit-scrollbar{width:4px}
.nav-scroll::-webkit-scrollbar-thumb{background:var(--card-border);border-radius:2px}
details.tree-svc,details.tree-mod,details.tree-section{margin-bottom:2px}
.tree-svc-title{font-size:12px;font-weight:700;padding:8px;cursor:pointer;list-style:none;display:flex;align-items:center;justify-content:space-between;border-radius:6px;color:var(--fg);user-select:none}
.tree-svc-title:hover{background:var(--table-hover)}
.tree-svc-title::-webkit-details-marker{display:none}
details.tree-svc>summary::before{content:"▸";margin-right:6px;font-size:10px;transition:transform var(--transition);display:inline-block}
details.tree-svc[open]>summary::before{transform:rotate(90deg)}
.tree-mod-title{font-size:11px;font-weight:600;padding:5px 8px 5px 20px;cursor:pointer;list-style:none;display:flex;align-items:center;justify-content:space-between;border-radius:4px;color:var(--muted);user-select:none}
.tree-mod-title:hover{background:var(--table-hover);color:var(--fg)}
.tree-mod-title::-webkit-details-marker{display:none}
details.tree-mod>summary::before{content:"▸";margin-right:4px;font-size:9px;transition:transform var(--transition);display:inline-block}
details.tree-mod[open]>summary::before{transform:rotate(90deg)}
.tree-section-title{font-size:11px;font-weight:700;text-transform:uppercase;letter-spacing:.05em;padding:10px 8px 5px;cursor:pointer;list-style:none;color:var(--muted-light);display:flex;align-items:center;justify-content:space-between;user-select:none}
.tree-section-title::-webkit-details-marker{display:none}
details.tree-section>summary::before{content:"▸";margin-right:6px;font-size:9px;transition:transform var(--transition);display:inline-block}
details.tree-section[open]>summary::before{transform:rotate(90deg)}
.badge{font-size:10px;background:var(--code-bg);color:var(--muted);padding:1px 7px;border-radius:10px;font-weight:600}
.nav-link{display:block;padding:4px 8px 4px 32px;font-size:12px;color:var(--fg);text-decoration:none;border-radius:4px;transition:all var(--transition);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.nav-link:hover{background:var(--accent-bg);color:var(--accent-fg)}
.nav-link .method-dot{display:inline-block;width:7px;height:7px;border-radius:50%;margin-right:6px;vertical-align:middle}
.nav-link.active{background:var(--accent-bg);color:var(--accent-fg);font-weight:600}
.main{margin-left:280px;flex:1;padding:24px 48px 48px;max-width:960px}
.breadcrumbs{position:sticky;top:0;background:var(--bg);padding:10px 0;margin-bottom:16px;font-size:12px;color:var(--muted);z-index:10;border-bottom:1px solid var(--card-border)}
.bc-item{color:var(--muted)}.bc-sep{margin:0 6px;color:var(--muted-light)}
.stats{display:flex;gap:16px;margin-bottom:28px;flex-wrap:wrap}
.stat{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);padding:14px 20px;box-shadow:var(--card-shadow);flex:1;min-width:100px;text-align:center}
.stat-value{font-size:26px;font-weight:700;color:var(--accent)}
.stat-label{font-size:11px;color:var(--muted);margin-top:2px}
.service-section{margin-bottom:32px}
.service-title{font-size:22px;font-weight:700;margin:32px 0 4px;letter-spacing:-.02em;display:flex;align-items:center;gap:12px}
.base-url{font-size:12px;font-weight:400;color:var(--muted);background:var(--code-bg);padding:2px 8px;border-radius:4px}
.service-stats{display:flex;gap:16px;margin-bottom:20px;font-size:12px;color:var(--muted)}
.service-stats span{background:var(--code-bg);padding:3px 10px;border-radius:12px}
.section-desc{color:var(--muted);font-size:13px;margin-bottom:16px}
.module-title{font-size:17px;font-weight:700;margin:28px 0 6px;display:flex;align-items:center;gap:8px}
.prefix-badge{font-size:11px;font-weight:400;color:var(--muted);background:var(--code-bg);padding:2px 8px;border-radius:4px}
.section-divider{font-size:22px;font-weight:700;margin:48px 0 16px;padding-top:16px;border-top:2px solid var(--card-border);letter-spacing:-.02em}
.endpoint{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);margin-bottom:10px;overflow:hidden;box-shadow:var(--card-shadow);transition:box-shadow var(--transition)}
.endpoint:hover{box-shadow:0 4px 12px rgba(0,0,0,.08)}
.endpoint.deprecated{opacity:.7}
.endpoint-header{display:flex;align-items:center;gap:10px;padding:12px 16px;cursor:pointer;user-select:none}
.endpoint-header:hover{background:var(--table-hover)}
.method-badge{display:inline-flex;align-items:center;justify-content:center;min-width:52px;padding:3px 8px;border-radius:4px;font-size:10px;font-weight:700;letter-spacing:.03em;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.method-GET{background:var(--get-bg);color:var(--get)}
.method-POST{background:var(--post-bg);color:var(--post)}
.method-PUT{background:var(--put-bg);color:var(--put)}
.method-DELETE{background:var(--delete-bg);color:var(--delete)}
.method-PATCH{background:var(--patch-bg);color:var(--patch)}
.method-WS{background:var(--ws-bg);color:var(--ws)}
.endpoint-path{font-family:'SF Mono',SFMono-Regular,Consolas,monospace;font-size:12px;font-weight:500;flex:1}
.endpoint-path .param{color:var(--accent);font-weight:600}
.endpoint-name{font-size:11px;color:var(--muted);font-weight:500;display:flex;align-items:center;gap:6px}
.chevron{font-size:10px;color:var(--muted-light);transition:transform var(--transition)}
.endpoint.open .chevron{transform:rotate(90deg)}
.endpoint-detail{padding:0 16px 14px;display:none;border-top:1px solid var(--card-border)}
.endpoint.open .endpoint-detail{display:block;padding-top:14px}
.detail-row{display:flex;gap:8px;margin-bottom:6px;font-size:12px;align-items:baseline}
.detail-label{font-weight:600;min-width:80px;color:var(--muted);flex-shrink:0}
.detail-value code{background:var(--code-bg);color:var(--code-fg);padding:2px 6px;border-radius:4px;font-size:11px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.deprecated-badge{font-size:9px;background:var(--put-bg);color:var(--put);padding:1px 6px;border-radius:3px;font-weight:700;text-transform:uppercase;letter-spacing:.03em}
.inline-fields{margin:8px 0;border:1px solid var(--card-border);border-radius:6px;overflow:hidden}
.inline-fields-title{font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:.04em;padding:6px 12px;background:var(--table-stripe);color:var(--muted)}
.type-link{color:var(--accent);text-decoration:none;font-weight:600;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace;background:var(--accent-bg);padding:1px 6px;border-radius:4px;transition:all var(--transition)}
.type-link:hover{color:var(--accent-hover);text-decoration:underline}
.model-card{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);margin-bottom:14px;overflow:hidden;box-shadow:var(--card-shadow)}
.model-header{padding:14px 16px;border-bottom:1px solid var(--card-border)}
.model-header h3{font-size:15px;font-weight:600;display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.extends{font-weight:400;color:var(--muted);font-size:12px}
.model-desc{color:var(--muted);font-size:12px;margin-top:4px}
.used-by{font-size:11px;color:var(--muted-light);margin-top:6px}
.used-by-action{background:var(--code-bg);padding:1px 6px;border-radius:3px;font-size:10px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.model-table{width:100%;border-collapse:collapse}
.model-table th{text-align:left;padding:8px 16px;font-size:10px;font-weight:600;text-transform:uppercase;letter-spacing:.05em;color:var(--muted-light);background:var(--table-stripe);border-bottom:1px solid var(--card-border)}
.model-table td{padding:8px 16px;font-size:12px;border-bottom:1px solid var(--card-border)}
.model-table tr:last-child td{border-bottom:none}
.model-table tr:hover td{background:var(--table-hover)}
.model-table code{background:var(--code-bg);color:var(--code-fg);padding:1px 5px;border-radius:3px;font-size:11px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.field-deprecated td{opacity:.6;text-decoration:line-through}
.optional-badge{display:inline-block;padding:1px 5px;border-radius:3px;font-size:9px;font-weight:700;background:var(--put-bg);color:var(--put)}
.attr-badge{display:inline-block;padding:1px 5px;border-radius:3px;font-size:9px;font-weight:600;background:var(--code-bg);color:var(--muted)}
.default-value{color:var(--accent);font-size:11px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.enum-values{display:flex;flex-wrap:wrap;gap:6px;padding:12px 16px}
.enum-value{background:var(--code-bg);color:var(--code-fg);padding:4px 10px;border-radius:4px;font-size:11px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace;font-weight:500}
@keyframes flash{0%{background:var(--highlight)}100%{background:transparent}}
.highlight{animation:flash 1.5s ease-out}
.footer{text-align:center;padding:48px 0 24px;color:var(--muted-light);font-size:11px}
.toolbar{position:fixed;top:12px;right:20px;z-index:30}
.theme-btn{background:var(--card);border:1px solid var(--card-border);color:var(--fg);width:34px;height:34px;border-radius:var(--radius);cursor:pointer;display:flex;align-items:center;justify-content:center;font-size:14px;box-shadow:var(--card-shadow);transition:all var(--transition)}
.theme-btn:hover{border-color:var(--accent);color:var(--accent)}
@media(max-width:768px){.sidebar{display:none}.main{margin-left:0;padding:16px}}
</style>
</head>
<body>
`

// ── JavaScript ──────────────────────────────────────────────────────────────

const htmlScript = `<script>
function toggleTheme(){var d=document.documentElement.getAttribute('data-theme')==='dark';document.documentElement.setAttribute('data-theme',d?'':'dark');localStorage.setItem('veld-theme',d?'':'dark')}
(function(){var t=localStorage.getItem('veld-theme');if(t)document.documentElement.setAttribute('data-theme',t)})();
function toggleDetail(h){h.closest('.endpoint').classList.toggle('open')}
function navigateTo(id){var el=document.getElementById(id);if(!el)return;el.scrollIntoView({behavior:'smooth',block:'start'});el.classList.remove('highlight');void el.offsetWidth;el.classList.add('highlight');var bc=el.getAttribute('data-bc');if(!bc){var p=el.closest('[data-bc]');if(p)bc=p.getAttribute('data-bc')}if(bc)setBreadcrumbs(bc);if(el.classList.contains('endpoint'))el.classList.add('open')}
function setBreadcrumbs(t){var el=document.getElementById('breadcrumbs');if(!el)return;var p=t.split(' \u203a ');el.innerHTML=p.map(function(s){return'<span class="bc-item">'+s+'</span>'}).join('<span class="bc-sep">\u203a</span>')}
(function(){var targets=document.querySelectorAll('[data-bc]');if(!targets.length)return;var obs=new IntersectionObserver(function(entries){for(var i=0;i<entries.length;i++){if(entries[i].isIntersecting){var bc=entries[i].target.getAttribute('data-bc');if(bc)setBreadcrumbs(bc);break}}},{rootMargin:'-80px 0px -70% 0px',threshold:0});targets.forEach(function(t){obs.observe(t)})})();
function filterNav(q){q=q.toLowerCase().trim();var links=document.querySelectorAll('.nav-link');var groups=document.querySelectorAll('details.tree-svc,details.tree-mod,details.tree-section');if(!q){links.forEach(function(a){a.style.display=''});groups.forEach(function(g){g.style.display='';g.open=true});return}links.forEach(function(a){var t=(a.getAttribute('data-search')||a.textContent).toLowerCase();a.style.display=t.indexOf(q)>=0?'':'none'});groups.forEach(function(g){var has=false;g.querySelectorAll('.nav-link').forEach(function(a){if(a.style.display!=='none')has=true});g.style.display=has?'':'none';if(has)g.open=true})}
(function(){var links=document.querySelectorAll('.nav-link');var secs=[];links.forEach(function(a){var h=a.getAttribute('href');if(h&&h.charAt(0)==='#'){var t=document.getElementById(h.slice(1));if(t)secs.push({link:a,target:t})}});if(!secs.length)return;var obs=new IntersectionObserver(function(entries){entries.forEach(function(e){var item;for(var i=0;i<secs.length;i++){if(secs[i].target===e.target){item=secs[i];break}}if(item&&e.isIntersecting){links.forEach(function(l){l.classList.remove('active')});item.link.classList.add('active')}})},{rootMargin:'-80px 0px -70% 0px',threshold:0});secs.forEach(function(s){obs.observe(s.target)})})();
</script>
`
