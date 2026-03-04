package docsgen

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// BuildHTML generates a standalone HTML API documentation page.
func BuildHTML(a ast.AST) string {
	var sb strings.Builder

	sb.WriteString(`<!DOCTYPE html>
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
  --radius:8px;--transition:150ms ease;
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
}
html{scroll-behavior:smooth}
body{font-family:Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6;display:flex;min-height:100vh}
.sidebar{width:280px;background:var(--sidebar-bg);border-right:1px solid var(--sidebar-border);position:fixed;top:0;left:0;height:100vh;display:flex;flex-direction:column;z-index:20}
.sidebar-header{padding:24px 20px 16px;border-bottom:1px solid var(--sidebar-border)}
.sidebar-header h1{font-size:20px;font-weight:700;letter-spacing:-.02em}
.sidebar-header h1 span{color:var(--accent)}
.sidebar-header p{font-size:12px;color:var(--muted);margin-top:2px}
.search-box{padding:12px 16px}
.search-box input{width:100%;padding:8px 12px 8px 36px;border:1px solid var(--card-border);border-radius:var(--radius);font-size:13px;background:var(--bg);color:var(--fg);outline:none;transition:border-color var(--transition)}
.search-box input:focus{border-color:var(--accent)}
.search-box{position:relative}
.search-box::before{content:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' fill='%236b7280' viewBox='0 0 24 24'%3E%3Cpath d='M21 21l-4.35-4.35M11 19a8 8 0 100-16 8 8 0 000 16z' stroke='%236b7280' stroke-width='2' fill='none' stroke-linecap='round'/%3E%3C/svg%3E");position:absolute;left:28px;top:50%;transform:translateY(-50%)}
.nav-scroll{flex:1;overflow-y:auto;padding:8px 12px 24px}
.nav-group{margin-bottom:16px}
.nav-group-title{font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.05em;color:var(--muted-light);padding:4px 8px;margin-bottom:4px}
.nav-link{display:block;padding:6px 12px;font-size:13px;color:var(--fg);text-decoration:none;border-radius:6px;transition:all var(--transition);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.nav-link:hover{background:var(--accent-bg);color:var(--accent-fg)}
.nav-link .method-dot{display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:8px;vertical-align:middle}
.main{margin-left:280px;flex:1;padding:48px 56px;max-width:960px}
.main h2{font-size:24px;font-weight:700;margin:48px 0 8px;letter-spacing:-.02em}
.main h2:first-child{margin-top:0}
.section-desc{color:var(--muted);font-size:14px;margin-bottom:24px}
.endpoint{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);margin-bottom:12px;overflow:hidden;box-shadow:var(--card-shadow);transition:box-shadow var(--transition)}
.endpoint:hover{box-shadow:0 4px 12px rgba(0,0,0,.08)}
.endpoint-header{display:flex;align-items:center;gap:12px;padding:14px 20px;cursor:pointer;user-select:none}
.endpoint-header:hover{background:var(--table-hover)}
.method-badge{display:inline-flex;align-items:center;justify-content:center;min-width:56px;padding:4px 10px;border-radius:4px;font-size:11px;font-weight:700;letter-spacing:.03em;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.method-GET{background:var(--get-bg);color:var(--get)}
.method-POST{background:var(--post-bg);color:var(--post)}
.method-PUT{background:var(--put-bg);color:var(--put)}
.method-DELETE{background:var(--delete-bg);color:var(--delete)}
.method-PATCH{background:var(--patch-bg);color:var(--patch)}
.method-WS{background:var(--ws-bg);color:var(--ws)}
.endpoint-path{font-family:'SF Mono',SFMono-Regular,Consolas,monospace;font-size:13px;font-weight:500;flex:1}
.endpoint-path .param{color:var(--accent);font-weight:600}
.endpoint-name{font-size:12px;color:var(--muted);font-weight:500}
.endpoint-detail{padding:0 20px 16px;display:none;border-top:1px solid var(--card-border)}
.endpoint-detail.open{display:block;padding-top:16px}
.detail-row{display:flex;gap:8px;margin-bottom:8px;font-size:13px}
.detail-label{font-weight:600;min-width:80px;color:var(--muted)}
.detail-value code{background:var(--code-bg);color:var(--code-fg);padding:2px 8px;border-radius:4px;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.model-card{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);margin-bottom:16px;overflow:hidden;box-shadow:var(--card-shadow)}
.model-header{padding:16px 20px;border-bottom:1px solid var(--card-border)}
.model-header h3{font-size:16px;font-weight:600}
.model-header h3 .extends{font-weight:400;color:var(--muted);font-size:13px;margin-left:8px}
.model-header .model-desc{color:var(--muted);font-size:13px;margin-top:4px}
.model-table{width:100%;border-collapse:collapse}
.model-table th{text-align:left;padding:10px 20px;font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.05em;color:var(--muted-light);background:var(--table-stripe);border-bottom:1px solid var(--card-border)}
.model-table td{padding:10px 20px;font-size:13px;border-bottom:1px solid var(--card-border)}
.model-table tr:last-child td{border-bottom:none}
.model-table tr:hover td{background:var(--table-hover)}
.model-table code{background:var(--code-bg);color:var(--code-fg);padding:2px 6px;border-radius:4px;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.optional-badge{display:inline-block;padding:1px 6px;border-radius:3px;font-size:10px;font-weight:600;background:var(--put-bg);color:var(--put)}
.default-value{color:var(--accent);font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.enum-values{display:flex;flex-wrap:wrap;gap:6px;padding:16px 20px}
.enum-value{background:var(--code-bg);color:var(--code-fg);padding:4px 12px;border-radius:4px;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace;font-weight:500}
.toolbar{position:fixed;top:16px;right:24px;z-index:30;display:flex;gap:8px}
.theme-btn{background:var(--card);border:1px solid var(--card-border);color:var(--fg);width:36px;height:36px;border-radius:var(--radius);cursor:pointer;display:flex;align-items:center;justify-content:center;font-size:16px;box-shadow:var(--card-shadow);transition:all var(--transition)}
.theme-btn:hover{border-color:var(--accent);color:var(--accent)}
.stats{display:flex;gap:24px;margin-bottom:32px}
.stat{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);padding:16px 24px;box-shadow:var(--card-shadow);flex:1;text-align:center}
.stat-value{font-size:28px;font-weight:700;color:var(--accent)}
.stat-label{font-size:12px;color:var(--muted);margin-top:2px}
@media(max-width:768px){.sidebar{display:none}.main{margin-left:0;padding:24px 16px}}
</style>
</head>
<body>
<div class="toolbar"><button class="theme-btn" onclick="toggleTheme()" title="Toggle dark mode"><svg id="theme-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg></button></div>
<nav class="sidebar">
  <div class="sidebar-header"><h1><span>Veld</span> API</h1><p>Auto-generated documentation</p></div>
  <div class="search-box"><input type="text" id="search" placeholder="Search endpoints..." oninput="filterNav(this.value)"></div>
  <div class="nav-scroll">
`)

	// Sidebar
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("    <div class=\"nav-group\"><div class=\"nav-group-title\">%s</div>\n", mod.Name))
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			dotColor := methodColor(act.Method)
			sb.WriteString(fmt.Sprintf("      <a href=\"#action-%s-%s\" class=\"nav-link\"><span class=\"method-dot\" style=\"background:%s\"></span>%s <span style=\"color:var(--muted-light);font-size:11px;margin-left:4px\">%s</span></a>\n",
				strings.ToLower(mod.Name), strings.ToLower(act.Name), dotColor, act.Name, routePath))
		}
		sb.WriteString("    </div>\n")
	}
	if len(a.Models) > 0 {
		sb.WriteString("    <div class=\"nav-group\"><div class=\"nav-group-title\">Models</div>\n")
		for _, m := range a.Models {
			sb.WriteString(fmt.Sprintf("      <a href=\"#model-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(m.Name), m.Name))
		}
		sb.WriteString("    </div>\n")
	}
	if len(a.Enums) > 0 {
		sb.WriteString("    <div class=\"nav-group\"><div class=\"nav-group-title\">Enums</div>\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("      <a href=\"#enum-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(en.Name), en.Name))
		}
		sb.WriteString("    </div>\n")
	}
	sb.WriteString("  </div>\n</nav>\n<main class=\"main\">\n")

	// Stats
	totalActions := 0
	for _, mod := range a.Modules {
		totalActions += len(mod.Actions)
	}
	sb.WriteString("<div class=\"stats\">\n")
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Modules</div></div>\n", len(a.Modules)))
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Endpoints</div></div>\n", totalActions))
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Models</div></div>\n", len(a.Models)))
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Enums</div></div>\n", len(a.Enums)))
	sb.WriteString("</div>\n\n")

	// Endpoint cards
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("<h2 id=\"mod-%s\">%s</h2>\n", strings.ToLower(mod.Name), mod.Name))
		if mod.Description != "" {
			sb.WriteString(fmt.Sprintf("<p class=\"section-desc\">%s</p>\n", mod.Description))
		}
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			method := strings.ToUpper(act.Method)
			highlightedPath := routePath
			for _, seg := range strings.Split(routePath, "/") {
				if strings.HasPrefix(seg, ":") {
					highlightedPath = strings.Replace(highlightedPath, seg, "<span class=\"param\">"+seg+"</span>", 1)
				}
			}
			sb.WriteString(fmt.Sprintf("<div class=\"endpoint\" id=\"action-%s-%s\">\n", strings.ToLower(mod.Name), strings.ToLower(act.Name)))
			sb.WriteString("  <div class=\"endpoint-header\" onclick=\"this.nextElementSibling.classList.toggle('open')\">\n")
			sb.WriteString(fmt.Sprintf("    <span class=\"method-badge method-%s\">%s</span>\n", method, method))
			sb.WriteString(fmt.Sprintf("    <span class=\"endpoint-path\">%s</span>\n", highlightedPath))
			sb.WriteString(fmt.Sprintf("    <span class=\"endpoint-name\">%s</span>\n", act.Name))
			sb.WriteString("  </div>\n  <div class=\"endpoint-detail\">\n")
			if act.Description != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Description</span><span>%s</span></div>\n", act.Description))
			}
			if act.Input != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Input</span><span class=\"detail-value\"><code>%s</code></span></div>\n", act.Input))
			}
			output := act.Output
			if act.OutputArray {
				output += "[]"
			}
			if output == "" {
				output = "void"
			}
			sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Output</span><span class=\"detail-value\"><code>%s</code></span></div>\n", output))
			if act.Query != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Query</span><span class=\"detail-value\"><code>%s</code></span></div>\n", act.Query))
			}
			if len(act.Middleware) > 0 {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Middleware</span><span class=\"detail-value\"><code>%s</code></span></div>\n", strings.Join(act.Middleware, ", ")))
			}
			sb.WriteString("  </div>\n</div>\n")
		}
		sb.WriteString("\n")
	}

	// Models
	if len(a.Models) > 0 {
		sb.WriteString("<h2>Models</h2>\n")
		for _, m := range a.Models {
			sb.WriteString(fmt.Sprintf("<div class=\"model-card\" id=\"model-%s\">\n  <div class=\"model-header\">\n    <h3>%s", strings.ToLower(m.Name), m.Name))
			if m.Extends != "" {
				sb.WriteString(fmt.Sprintf("<span class=\"extends\">extends %s</span>", m.Extends))
			}
			sb.WriteString("</h3>\n")
			if m.Description != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"model-desc\">%s</div>\n", m.Description))
			}
			sb.WriteString("  </div>\n")
			if len(m.Fields) > 0 {
				sb.WriteString("  <table class=\"model-table\"><thead><tr><th>Field</th><th>Type</th><th>Attributes</th></tr></thead><tbody>\n")
				for _, f := range m.Fields {
					typeName := f.Type
					if f.IsArray {
						typeName += "[]"
					}
					if f.IsMap {
						typeName = fmt.Sprintf("Map&lt;string, %s&gt;", f.MapValueType)
					}
					attrs := ""
					if f.Optional {
						attrs += "<span class=\"optional-badge\">optional</span> "
					}
					if f.Default != "" {
						attrs += fmt.Sprintf("<span class=\"default-value\">= %s</span>", f.Default)
					}
					if attrs == "" {
						attrs = "&mdash;"
					}
					sb.WriteString(fmt.Sprintf("    <tr><td><strong>%s</strong></td><td><code>%s</code></td><td>%s</td></tr>\n", f.Name, typeName, attrs))
				}
				sb.WriteString("  </tbody></table>\n")
			}
			sb.WriteString("</div>\n")
		}
		sb.WriteString("\n")
	}

	// Enums
	if len(a.Enums) > 0 {
		sb.WriteString("<h2>Enums</h2>\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("<div class=\"model-card\" id=\"enum-%s\">\n  <div class=\"model-header\">\n    <h3>%s</h3>\n", strings.ToLower(en.Name), en.Name))
			if en.Description != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"model-desc\">%s</div>\n", en.Description))
			}
			sb.WriteString("  </div>\n  <div class=\"enum-values\">\n")
			for _, v := range en.Values {
				sb.WriteString(fmt.Sprintf("    <span class=\"enum-value\">%s</span>\n", v))
			}
			sb.WriteString("  </div>\n</div>\n")
		}
		sb.WriteString("\n")
	}

	sb.WriteString(`<div style="text-align:center;padding:48px 0 24px;color:var(--muted-light);font-size:12px">Generated by Veld</div>
</main>
<script>
function toggleTheme(){const h=document.documentElement;const d=h.getAttribute('data-theme')==='dark';h.setAttribute('data-theme',d?'':'dark');localStorage.setItem('veld-theme',d?'':'dark')}
(function(){const t=localStorage.getItem('veld-theme');if(t)document.documentElement.setAttribute('data-theme',t)})();
function filterNav(q){q=q.toLowerCase();document.querySelectorAll('.nav-link').forEach(a=>{a.style.display=a.textContent.toLowerCase().includes(q)?'':'none'});document.querySelectorAll('.nav-group').forEach(g=>{const v=g.querySelectorAll('.nav-link[style=""],.nav-link:not([style])');g.style.display=v.length||!q?'':'none'})}
</script>
</body>
</html>
`)
	return sb.String()
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
