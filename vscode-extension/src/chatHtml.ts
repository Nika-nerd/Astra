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
        <span><strong>Astra</strong> · Gemma 4 + AST</span>
        <span id="wsBadge">Workspace</span>
    </header>

    <div id="chat">
        <div class="msg agent">
           Hi! I'm <strong>Agent Astra</strong>. I have full visibility into your Rust project's AST.<br/>
            Use <code>@filename</code> to reference files and describe your task — I will explain the plan in your prompt's language, generate the Rust code, verify it with <code>cargo clippy</code> and <code>cargo test</code>, and let you <strong>Apply</strong> the changes.
        </div>
    </div>

    <div id="liveLogs"></div>

    <div class="composer">
        <div id="mentionPopup"></div>
        <textarea id="promptInput" placeholder="Describe your task... (type @ to mention a file, Enter to send)"></textarea>
        <div class="hint">
            <span>@ to mention file</span>
            <span>Enter to send, Shift+Enter for newline<</span>
        </div>
        <button id="sendBtn">Send Request</button>
    </div>

    <script>
        const vscode = acquireVsCodeApi();
        const chatEl = document.getElementById('chat');
        const promptInput = document.getElementById('promptInput');
        const sendBtn = document.getElementById('sendBtn');
        const mentionPopup = document.getElementById('mentionPopup');
        const liveLogs = document.getElementById('liveLogs');
        const wsBadge = document.getElementById('wsBadge');

        let workspaceFiles = [];
        let pendingPatchesMap = {};
        let msgCounter = 0;

        vscode.postMessage({ type: 'requestFileList' });

        function escapeHtml(str) {
            return String(str)
                .replace(/&/g, '&amp;')
                .replace(/</g, '&lt;')
                .replace(/>/g, '&gt;');
        }

        function sendPrompt() {
            const text = promptInput.value.trim();
            if (!text || sendBtn.disabled) return;
            promptInput.value = '';
            mentionPopup.style.display = 'none';
            sendBtn.disabled = true;
            sendBtn.textContent = 'Astra verifying (clippy & test)...';
            liveLogs.style.display = 'block';
            liveLogs.textContent = '';
            vscode.postMessage({ type: 'runTask', prompt: text });
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
                if (msg.workspaceName) {
                    wsBadge.textContent = '📁 ' + msg.workspaceName;
                }
            } else if (msg.type === 'userMessage') {
                if (msg.workspaceName) {
                    wsBadge.textContent = '📁 ' + msg.workspaceName;
                }
                const div = document.createElement('div');
                div.className = 'msg user';
                let chipsHtml = '';
                if (msg.targetFiles && msg.targetFiles.length > 0) {
                    chipsHtml = '<div class="chips">' + msg.targetFiles.map(f => '<span class="chip">@' + escapeHtml(f) + '</span>').join('') + '</div>';
                }
                div.innerHTML = chipsHtml + '<div>' + escapeHtml(msg.text) + '</div>';
                chatEl.appendChild(div);
                chatEl.scrollTop = chatEl.scrollHeight;
            } else if (msg.type === 'log') {
                liveLogs.style.display = 'block';
                liveLogs.textContent += msg.text + '\\n';
                liveLogs.scrollTop = liveLogs.scrollHeight;
            } else if (msg.type === 'agentResult') {
                sendBtn.disabled = false;
                sendBtn.textContent = 'Send Request';
                liveLogs.style.display = 'none';

                const res = msg.result;
                const msgId = 'msg-' + (++msgCounter);
                const div = document.createElement('div');
                div.className = 'msg agent';

                if (res.success) {
                    pendingPatchesMap[msgId] = res.proposed_patches;
                    let html = '<div class="explanation">' + escapeHtml(res.explanation) + '</div>';
                    html += '<div class="status-bar status-ok">✓ Verified: cargo clippy & cargo test (attempts: ' + res.attempts_used + ')</div>';

                    for (const p of res.proposed_patches) {
                        html += '<div class="patch-card">' +
                            '<div class="patch-header">📄 ' + escapeHtml(p.relative_path) + '</div>' +
                            '<pre class="patch-code">' + escapeHtml(p.content) + '</pre>' +
                            '</div>';
                    }

                    html += '<div class="actions"><button id="apply-' + msgId + '">✓ Apply Changes</button></div>';
                    div.innerHTML = html;
                    chatEl.appendChild(div);

                    const applyBtn = document.getElementById('apply-' + msgId);
                    applyBtn.addEventListener('click', () => {
                        vscode.postMessage({
                            type: 'applyPatches',
                            patches: pendingPatchesMap[msgId],
                            messageId: msgId
                        });
                    });
                } else {
                    let html = '';
                    if (res.explanation) {
                        html += '<div class="explanation">' + escapeHtml(res.explanation) + '</div>';
                    }
                    html += '<div class="status-bar status-err">✗ Failed clippy/test verification after ' + res.attempts_used + ' attempts</div>';
                    if (res.last_error) {
                        html += '<pre class="patch-code">' + escapeHtml(res.last_error) + '</pre>';
                    }
                    div.innerHTML = html;
                    chatEl.appendChild(div);
                }
                chatEl.scrollTop = chatEl.scrollHeight;
            } else if (msg.type === 'applied') {
                const btn = document.getElementById('apply-' + msg.messageId);
                if (btn) {
                    btn.disabled = true;
                    btn.textContent = '✓ Changes Applied';
                }
            } else if (msg.type === 'error') {
                sendBtn.disabled = false;
                sendBtn.textContent = 'Send Request';
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