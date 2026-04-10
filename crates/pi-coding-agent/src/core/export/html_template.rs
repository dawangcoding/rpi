//! HTML 模板定义
//!
//! 包含自包含 HTML 导出所需的 CSS 和 JS 模板

/// HTML 头部模板（包含占位符）
/// 占位符: {title}, {meta_info}, {theme_class}
pub const HTML_HEADER: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
{css_styles}
</style>
</head>
<body class="{theme_class}">
<div class="container">
<header>
<h1>{title}</h1>
<div class="meta">{meta_info}</div>
</header>
<div class="messages">
"##;

/// HTML 底部模板
pub const HTML_FOOTER: &str = r##"
</div>
<footer>
<div class="export-info">Exported with pi-coding-agent</div>
</footer>
</div>
<script>
{js_code}
</script>
</body>
</html>"##;

/// CSS 样式
pub const CSS_STYLES: &str = r##"
:root {
  --bg-color: #ffffff;
  --text-color: #1a1a1a;
  --border-color: #e1e4e8;
  --user-bg: #e3f2fd;
  --user-border: #2196f3;
  --assistant-bg: #f5f5f5;
  --assistant-border: #9e9e9e;
  --tool-bg: #fff3e0;
  --tool-border: #ff9800;
  --error-bg: #ffebee;
  --error-border: #f44336;
  --thinking-bg: #fafafa;
  --thinking-border: #bdbdbd;
  --code-bg: #263238;
  --code-text: #eeffff;
  --link-color: #1976d2;
  --meta-color: #666;
  --shadow: 0 1px 3px rgba(0,0,0,0.12);
}

.dark {
  --bg-color: #0d1117;
  --text-color: #c9d1d9;
  --border-color: #30363d;
  --user-bg: #1c3a5c;
  --user-border: #58a6ff;
  --assistant-bg: #161b22;
  --assistant-border: #8b949e;
  --tool-bg: #3d2817;
  --tool-border: #f0883e;
  --error-bg: #3d1f1f;
  --error-border: #f85149;
  --thinking-bg: #21262d;
  --thinking-border: #484f58;
  --code-bg: #161b22;
  --code-text: #c9d1d9;
  --link-color: #58a6ff;
  --meta-color: #8b949e;
  --shadow: 0 1px 3px rgba(0,0,0,0.3);
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  background-color: var(--bg-color);
  color: var(--text-color);
  line-height: 1.6;
  min-height: 100vh;
}

.container {
  max-width: 900px;
  margin: 0 auto;
  padding: 20px;
}

header {
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 20px;
  margin-bottom: 30px;
}

header h1 {
  font-size: 1.8rem;
  font-weight: 600;
  margin-bottom: 8px;
  word-break: break-word;
}

.meta {
  color: var(--meta-color);
  font-size: 0.9rem;
}

.messages {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.message {
  border-radius: 12px;
  padding: 16px;
  box-shadow: var(--shadow);
  border-left: 4px solid transparent;
}

.message.user {
  background-color: var(--user-bg);
  border-left-color: var(--user-border);
}

.message.assistant {
  background-color: var(--assistant-bg);
  border-left-color: var(--assistant-border);
}

.message.tool-result {
  background-color: var(--tool-bg);
  border-left-color: var(--tool-border);
}

.message.tool-result.error {
  background-color: var(--error-bg);
  border-left-color: var(--error-border);
}

.role {
  font-weight: 600;
  font-size: 0.85rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
  opacity: 0.8;
}

.content {
  font-size: 0.95rem;
}

.content p {
  margin-bottom: 12px;
}

.content p:last-child {
  margin-bottom: 0;
}

.content pre {
  background-color: var(--code-bg);
  color: var(--code-text);
  padding: 16px;
  border-radius: 8px;
  overflow-x: auto;
  font-family: "SF Mono", Monaco, "Cascadia Code", "Roboto Mono", Consolas, monospace;
  font-size: 0.85rem;
  line-height: 1.5;
  margin: 12px 0;
}

.content code {
  background-color: var(--code-bg);
  color: var(--code-text);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: "SF Mono", Monaco, "Cascadia Code", "Roboto Mono", Consolas, monospace;
  font-size: 0.9em;
}

.content pre code {
  padding: 0;
  background: none;
}

.content ul, .content ol {
  margin: 12px 0;
  padding-left: 24px;
}

.content li {
  margin: 4px 0;
}

.content blockquote {
  border-left: 4px solid var(--border-color);
  padding-left: 16px;
  margin: 12px 0;
  color: var(--meta-color);
}

.content a {
  color: var(--link-color);
  text-decoration: none;
}

.content a:hover {
  text-decoration: underline;
}

.content table {
  width: 100%;
  border-collapse: collapse;
  margin: 12px 0;
}

.content th, .content td {
  border: 1px solid var(--border-color);
  padding: 8px 12px;
  text-align: left;
}

.content th {
  background-color: var(--assistant-bg);
  font-weight: 600;
}

/* Thinking block */
details.thinking {
  background-color: var(--thinking-bg);
  border: 1px solid var(--thinking-border);
  border-radius: 8px;
  margin: 12px 0;
  overflow: hidden;
}

details.thinking summary {
  padding: 12px 16px;
  cursor: pointer;
  font-weight: 500;
  color: var(--meta-color);
  user-select: none;
}

details.thinking summary:hover {
  background-color: rgba(0,0,0,0.02);
}

details.thinking pre {
  margin: 0;
  padding: 16px;
  background-color: rgba(0,0,0,0.02);
  border-top: 1px solid var(--thinking-border);
  font-family: "SF Mono", Monaco, monospace;
  font-size: 0.85rem;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-color);
}

/* Tool call block */
details.tool-call {
  background-color: var(--tool-bg);
  border: 1px solid var(--tool-border);
  border-radius: 8px;
  margin: 12px 0;
  overflow: hidden;
}

details.tool-call summary {
  padding: 12px 16px;
  cursor: pointer;
  font-weight: 500;
  user-select: none;
}

details.tool-call summary:hover {
  background-color: rgba(0,0,0,0.02);
}

details.tool-call pre {
  margin: 0;
  padding: 16px;
  background-color: var(--code-bg);
  border-top: 1px solid var(--tool-border);
}

details.tool-call code {
  color: var(--code-text);
}

/* Image placeholder */
.image-placeholder {
  display: inline-block;
  padding: 8px 16px;
  background-color: var(--thinking-bg);
  border: 1px dashed var(--thinking-border);
  border-radius: 8px;
  color: var(--meta-color);
  font-size: 0.9rem;
  margin: 8px 0;
}

/* Stats section */
.stats {
  background-color: var(--assistant-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 16px;
  margin-top: 24px;
}

.stats h3 {
  font-size: 1rem;
  margin-bottom: 12px;
  font-weight: 600;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 12px;
}

.stat-item {
  display: flex;
  justify-content: space-between;
  font-size: 0.9rem;
}

.stat-label {
  color: var(--meta-color);
}

.stat-value {
  font-weight: 500;
}

/* Footer */
footer {
  margin-top: 40px;
  padding-top: 20px;
  border-top: 1px solid var(--border-color);
  text-align: center;
}

.export-info {
  color: var(--meta-color);
  font-size: 0.85rem;
}

/* Theme toggle */
.theme-toggle {
  position: fixed;
  top: 20px;
  right: 20px;
  background-color: var(--assistant-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 0.9rem;
  color: var(--text-color);
  box-shadow: var(--shadow);
  transition: all 0.2s;
}

.theme-toggle:hover {
  background-color: var(--thinking-bg);
}

/* Responsive */
@media (max-width: 600px) {
  .container {
    padding: 12px;
  }
  
  header h1 {
    font-size: 1.4rem;
  }
  
  .message {
    padding: 12px;
  }
  
  .theme-toggle {
    top: 10px;
    right: 10px;
    padding: 6px 12px;
    font-size: 0.8rem;
  }
}
"##;

/// JavaScript 代码
pub const JS_CODE: &str = r##"
// Theme toggle functionality
(function() {
  const body = document.body;
  const isDark = body.classList.contains('dark');
  
  // Create theme toggle button
  const toggle = document.createElement('button');
  toggle.className = 'theme-toggle';
  toggle.textContent = isDark ? '☀️ Light' : '🌙 Dark';
  toggle.setAttribute('aria-label', 'Toggle theme');
  document.body.appendChild(toggle);
  
  toggle.addEventListener('click', function() {
    const isCurrentlyDark = body.classList.contains('dark');
    if (isCurrentlyDark) {
      body.classList.remove('dark');
      toggle.textContent = '🌙 Dark';
      localStorage.setItem('pi-theme', 'light');
    } else {
      body.classList.add('dark');
      toggle.textContent = '☀️ Light';
      localStorage.setItem('pi-theme', 'dark');
    }
  });
  
  // Restore theme preference
  const savedTheme = localStorage.getItem('pi-theme');
  if (savedTheme === 'dark' && !body.classList.contains('dark')) {
    body.classList.add('dark');
    toggle.textContent = '☀️ Light';
  } else if (savedTheme === 'light' && body.classList.contains('dark')) {
    body.classList.remove('dark');
    toggle.textContent = '🌙 Dark';
  }
})();

// Auto-expand thinking blocks on search
(function() {
  if (window.location.hash) {
    const id = window.location.hash.slice(1);
    const element = document.getElementById(id);
    if (element) {
      const details = element.closest('details');
      if (details) {
        details.open = true;
      }
    }
  }
})();
"##;
