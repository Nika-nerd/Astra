export function getChatHtml(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
    * { box-sizing: border-box; }
    body {
        font-family: var(--vscode-font-family);
        color: var(--vscode-foreground);
        background-color: var(--vscode-sideBar-background);
        margin: 0;
        padding: 0;
        display: flex;
        flex-direction: column;
        height: 100vh;
    }
    header {
        padding: 8px 12px;
        border-bottom: 1px solid var(--vscode-panel-border);
        font-size: 11px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        opacity: 0.85;
    }
    #chat {
        flex: 1;
        overflow-y: auto;
        padding: 12px;
        display: flex;
        flex-direction: column;
        gap: 12px;
    }
    .msg {
        border-radius: 6px;
        padding: 10px;
        font-size: 12.5px;
        line-height: 1.45;
        word-break: break-word;
    }
    .msg.user {
        background: var(--vscode-input-background);
        border: 1px solid var(--vscode-input-border, transparent);
        align-self: flex-end;
        width: 92%;
    }
    .msg.agent {
        background: var(--vscode-editor-background);
        border: 1px solid var(--vscode-panel-border);
        width: 100%;
    }
    .chips {
        display: flex;
        flex-wrap: wrap;
        gap: 4px;
        margin-bottom: 6px;
    }
    .chip {
        font-size: 10.5px;
        background: var(--vscode-badge-background);
        color: var(--vscode-badge-foreground);
        padding: 2px 6px;
        border-radius: 3px;
        font-family: monospace;
    }
    .explanation {
        white-space: pre-wrap;
        margin-bottom: 10px;
    }
    .status-bar {
        font-size: 11px;
        padding: 6px 8px;
        border-radius: 4px;
        margin-bottom: 8px;
        font-family: monospace;
    }
    .status-ok {
        background: rgba(78, 201, 176, 0.12);
        color: #4ec9b0;
        border: 1px solid rgba(78, 201, 176, 0.3);
    }
    .status-err {
        background: rgba(241, 76, 76, 0.12);
        color: #f14c4c;
        border: 1px solid rgba(241, 76, 76, 0.3);
    }
    .patch-card {
        border: 1px solid var(--vscode-panel-border);
        border-radius: 4px;
        margin-top: 8px;
        overflow: hidden;
    }
    .patch-header {
        background: var(--vscode-sideBarSectionHeader-background);
        padding: 5px 8px;
        font-family: monospace;
        font-size: 11px;
        font-weight: 600;
    }
    .patch-code {
        margin: 0;
        padding: 8px;
        font-family: var(--vscode-editor-font-family, monospace);
        font-size: 11px;
        max-height: 220px;
        overflow: auto;
        background: var(--vscode-editor-background);
    }
    .actions {
        margin-top: 10px;
        display: flex;
        gap: 8px;
    }
    button {
        background: var(--vscode-button-background);
        color: var(--vscode-button-foreground);
        border: none;
        padding: 6px 12px;
        border-radius: 4px;
        cursor: pointer;
        font-weight: 600;
        font-size: 12px;
    }
    button:hover { background: var(--vscode-button-hoverBackground); }
    button:disabled { opacity: 0.5; cursor: default; }
    #liveLogs {
        font-family: monospace;
        font-size: 10.5px;
        opacity: 0.75;
        padding: 6px 12px;
        border-top: 1px solid var(--vscode-panel-border);
        max-height: 85px;
        overflow-y: auto;
        white-space: pre-wrap;
        display: none;
    }
    .composer {
        padding: 10px;
        border-top: 1px solid var(--vscode-panel-border);
        position: relative;
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    #mentionPopup {
        position: absolute;
        bottom: 100%;
        left: 10px;
        right: 10px;
        background: var(--vscode-editorWidget-background);
        border: 1px solid var(--vscode-editorWidget-border, var(--vscode-panel-border));
        border-radius: 4px;
        max-height: 160px;
        overflow-y: auto;
        display: none;
        z-index: 20;
    }
    .mention-item {
        padding: 5px 8px;
        font-family: monospace;
        font-size: 11.5px;
        cursor: pointer;
    }
    .mention-item:hover {
        background: var(--vscode-list-activeSelectionBackground);
        color: var(--vscode-list-activeSelectionForeground);
    }
    textarea {
        width: 100%;
        background: var(--vscode-input-background);
        color: var(--vscode-input-foreground);
        border: 1px solid var(--vscode-input-border, transparent);
        padding: 8px;
        border-radius: 4px;
        font-family: inherit;
        font-size: 12px;
        resize: none;
        min-height: 68px;
    }
     .hint {
        font-size: 10.5px;
        opacity: 0.65;
        display: flex;
        justify-content: space-between;
    }
</style>
</head>
<body>
    <header>
        <span><strong>Agent Astra</strong> · Gemma 4 + AST</span>
        <span id="wsBadge">Workspace</span>
    </header>

    <div id="chat">
        <div class="msg agent">
            Hi! I'm <strong>Agent Astra</strong>. I have full visibility into your Rust project's AST.<br/>
            1. Describe your project or task (use <code>@filename</code> to reference files).<br/>
            2. Review or refine the generated <strong>Plan</strong>.<br/>
            3. Click <strong>Apply Plan &amp; Build</strong> — I will autonomously run Cargo commands, create files, and verify everything with <code>cargo clippy</code> and <code>cargo test</code>.
        </div>
    </div>

    <div id="liveLogs"></div>

    <div class="composer">
        <div id="mentionPopup"></div>
        <textarea id="promptInput" placeholder="Describe project or task... (or type feedback to refine active plan)"></textarea>
        <div class="hint">
            <span id="modeHint">Mode: New Plan (@ to mention file)</span>
            <span>Enter to send</span>
        </div>
        <button id="sendBtn">Generate Plan</button>
    </div>

    <script>
        const vscode = acquireVsCodeApi();
        const chatEl = document.getElementById('chat');
        const promptInput = document.getElementById('promptInput');
        const sendBtn = document.getElementById('sendBtn');
        const mentionPopup = document.getElementById('mentionPopup');
        const liveLogs = document.getElementById('liveLogs');
        const wsBadge = document.getElementById('wsBadge');
        const modeHint = document.getElementById('modeHint');

        let workspaceFiles = [];
        let activePlanState = null;
        let msgCounter = 0;

        vscode.postMessage({ type: 'requestFileList' });

        function escapeHtml(str) {
            return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
        }

        function sendPrompt() {
            const text = promptInput.value.trim();
            if (!text || sendBtn.disabled) return;
            promptInput.value = '';
            mentionPopup.style.display = 'none';
            sendBtn.disabled = true;
            sendBtn.textContent = activePlanState ? 'Refining Plan...' : 'Generating Plan...';
            liveLogs.style.display = 'block';
            liveLogs.textContent = '';

            vscode.postMessage({
                type: 'runTask',
                prompt: text,
                previousPlan: activePlanState ? activePlanState.plan : undefined,
                targetFiles: activePlanState ? activePlanState.targetFiles : []
            });
        }

        sendBtn.addEventListener('click', sendPrompt);
        promptInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                sendPrompt();
            }
        });

        promptInput.addEventListener('input', () => {
            const val = promptInput.value;
            const cursorPos = promptInput.selectionStart;
            const beforeCursor = val.slice(0, cursorPos);
            const atMatch = beforeCursor.match(/@([\\w./-]*)$/);

            if (atMatch) {
                vscode.postMessage({ type: 'requestFileList' });
                const query = atMatch[1].toLowerCase();
                const matches = workspaceFiles.filter(f => f.toLowerCase().includes(query)).slice(0, 15);
                if (matches.length > 0) {
                    mentionPopup.innerHTML = matches
                        .map(f => '<div class="mention-item" data-file="' + escapeHtml(f) + '">@' + escapeHtml(f) + '</div>')
                        .join('');
                    mentionPopup.style.display = 'block';
                    mentionPopup.querySelectorAll('.mention-item').forEach(item => {
                        item.addEventListener('click', () => {
                            const chosen = item.getAttribute('data-file');
                            const replaced = beforeCursor.replace(/@([\\w./-]*)$/, '@' + chosen + ' ');
                            promptInput.value = replaced + val.slice(cursorPos);
                            mentionPopup.style.display = 'none';
                            promptInput.focus();
                        });
                    });
                    return;
                }
            }
            mentionPopup.style.display = 'none';
        });

        window.addEventListener('message', (event) => {
            const msg = event.data;

            if (msg.type === 'fileList') {
                workspaceFiles = msg.files || [];
                if (msg.workspaceName) wsBadge.textContent = '📁 ' + msg.workspaceName;
            } else if (msg.type === 'userMessage') {
                if (msg.workspaceName) wsBadge.textContent = '📁 ' + msg.workspaceName;
                const div = document.createElement('div');
                div.className = 'msg user';
                let chips = msg.isRefinement ? '<span class="chip">↻ Refine Plan</span> ' : '';
                if (msg.targetFiles && msg.targetFiles.length > 0) {
                    chips += msg.targetFiles.map(f => '<span class="chip">@' + escapeHtml(f) + '</span>').join(' ');
                }
                div.innerHTML = (chips ? '<div class="chips">' + chips + '</div>' : '') + '<div>' + escapeHtml(msg.text) + '</div>';
                chatEl.appendChild(div);
                chatEl.scrollTop = chatEl.scrollHeight;
            } else if (msg.type === 'log') {
                liveLogs.style.display = 'block';
                liveLogs.textContent += msg.text + '\\n';
                liveLogs.scrollTop = liveLogs.scrollHeight;
            } else if (msg.type === 'planResult') {
                sendBtn.disabled = false;
                liveLogs.style.display = 'none';
                const planText = msg.result.plan || '';
                const planId = 'plan-' + (++msgCounter);

                activePlanState = {
                    plan: planText,
                    originalPrompt: msg.originalPrompt,
                    targetFiles: msg.targetFiles || []
                };

                modeHint.textContent = 'Mode: Type feedback to refine plan, or click Apply below';
                sendBtn.textContent = 'Refine Plan';

                const div = document.createElement('div');
                div.className = 'msg agent';
                div.innerHTML =
                    '<div class="status-bar status-ok">📋 Proposed Architectural Plan</div>' +
                    '<div class="explanation">' + escapeHtml(planText) + '</div>' +
                    '<div class="actions">' +
                        '<button id="approve-' + planId + '">✓ Apply Plan &amp; Build Project</button>' +
                        '<button id="reset-' + planId + '" style="background:transparent;border:1px solid var(--vscode-panel-border);">New Task</button>' +
                    '</div>';
                chatEl.appendChild(div);
                chatEl.scrollTop = chatEl.scrollHeight;

                const approveBtn = document.getElementById('approve-' + planId);
                const resetBtn = document.getElementById('reset-' + planId);

                approveBtn.addEventListener('click', () => {
                    approveBtn.disabled = true;
                    approveBtn.textContent = 'Building & Verifying (clippy + test)...';
                    sendBtn.disabled = true;
                    liveLogs.style.display = 'block';
                    liveLogs.textContent = '';
                    vscode.postMessage({
                        type: 'executePlan',
                        originalPrompt: activePlanState.originalPrompt,
                        approvedPlan: activePlanState.plan,
                        targetFiles: activePlanState.targetFiles
                    });
                });

                resetBtn.addEventListener('click', () => {
                    activePlanState = null;
                    modeHint.textContent = 'Mode: New Plan (@ to mention file)';
                    sendBtn.textContent = 'Generate Plan';
                });
            } else if (msg.type === 'buildResult') {
                sendBtn.disabled = false;
                sendBtn.textContent = 'Generate Plan';
                modeHint.textContent = 'Mode: New Plan (@ to mention file)';
                liveLogs.style.display = 'none';
                activePlanState = null;

                const res = msg.result;
                const div = document.createElement('div');
                div.className = 'msg agent';

                if (res.success) {
                    let html = '<div class="explanation">' + escapeHtml(res.explanation) + '</div>';
                    html += '<div class="status-bar status-ok">✓ Built &amp; Verified by clippy + test (attempts: ' + res.attempts_used + ')</div>';
                    if (res.executed_commands && res.executed_commands.length > 0) {
                        html += '<div class="chips">' + res.executed_commands.map(c => '<span class="chip">$ ' + escapeHtml(c) + '</span>').join('') + '</div>';
                    }
                    for (const p of res.proposed_patches) {
                        html += '<div class="patch-card">' +
                            '<div class="patch-header" data-open="' + escapeHtml(p.relative_path) + '">📄 ' + escapeHtml(p.relative_path) + ' (Saved ✓)</div>' +
                            '<pre class="patch-code">' + escapeHtml(p.content) + '</pre></div>';
                    }
                    div.innerHTML = html;
                    chatEl.appendChild(div);
                    div.querySelectorAll('[data-open]').forEach(el => {
                        el.addEventListener('click', () => {
                            vscode.postMessage({ type: 'openFile', file: el.getAttribute('data-open') });
                        });
                    });
                } else {
                    let html = '<div class="status-bar status-err">✗ Build failed verification after ' + res.attempts_used + ' attempts (rolled back)</div>';
                    if (res.last_error) html += '<pre class="patch-code">' + escapeHtml(res.last_error) + '</pre>';
                    div.innerHTML = html;
                    chatEl.appendChild(div);
                }
                chatEl.scrollTop = chatEl.scrollHeight;
            } else if (msg.type === 'error') {
                sendBtn.disabled = false;
                sendBtn.textContent = activePlanState ? 'Refine Plan' : 'Generate Plan';
                liveLogs.style.display = 'none';
                const div = document.createElement('div');
                div.className = 'msg agent';
                div.innerHTML = '<div class="status-bar status-err">Error: ' + escapeHtml(msg.message) + '</div>';
                chatEl.appendChild(div);
                chatEl.scrollTop = chatEl.scrollHeight;
            }
        });
    </script>
</body>
</html>`;
}