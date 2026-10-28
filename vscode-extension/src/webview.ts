import * as vscode from 'vscode';
import * as path from 'path';
import { AstraBackendClient } from './client';

export class AstraChatViewProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = 'astra.chatView';
    private view?: vscode.WebviewView;

    constructor(
        private readonly extensionUri: vscode.Uri,
        private readonly client: AstraBackendClient
    ) {
        this.client.onLog((logLine: string) => {
            this.postMessage({ type: 'log', text: logLine });
        });
    }

    public resolveWebviewView(webviewView: vscode.WebviewView): void {
        this.view = webviewView;
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this.extensionUri]
        };

        webviewView.webview.html = this.getHtmlForWebview();

        webviewView.webview.onDidReceiveMessage(async (data: {
            type: string;
            prompt?: string;
            targetFiles?: string;
            file?: string;
        }) => {
            if (data.type === 'runTask' && data.prompt) {
                await this.handleRunTask(data.prompt, data.targetFiles || '');
            } else if (data.type === 'useActiveFile') {
                this.sendActiveFileToWebview();
            } else if (data.type === 'openFile' && data.file) {
                await this.openModifiedFile(data.file);
            }
        });
    }

    private sendActiveFileToWebview(): void {
        const editor = vscode.window.activeTextEditor;
        const root = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (editor && root) {
            const rel = path.relative(root, editor.document.uri.fsPath);
            this.postMessage({ type: 'activeFile', file: rel });
        }
    }

    private async openModifiedFile(relPath: string): Promise<void> {
        const root = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!root) {
            return;
        }
        const fullUri = vscode.Uri.file(path.join(root, relPath));
        const doc = await vscode.workspace.openTextDocument(fullUri);
        await vscode.window.showTextDocument(doc);
    }

    public async handleRunTask(prompt: string, rawTargetFiles: string): Promise<void> {
        const target_files = rawTargetFiles
            .split(',')
            .map((s) => s.trim())
            .filter((s) => s.length > 0);

        this.postMessage({ type: 'status', running: true, prompt });

        try {
            const result = await this.client.executeTask({
                prompt,
                target_files
            });
            this.postMessage({ type: 'result', result });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            this.postMessage({ type: 'error', message: msg });
        }
    }

    private postMessage(msg: unknown): void {
        this.view?.webview.postMessage(msg);
    }

    private getHtmlForWebview(): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
    body {
        font-family: var(--vscode-font-family);
        color: var(--vscode-foreground);
        background-color: var(--vscode-sideBar-background);
        padding: 12px;
        display: flex;
        flex-direction: column;
        gap: 10px;
    }
    label {
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        opacity: 0.8;
    }
    .row {
        display: flex;
        gap: 6px;
    }
    input, textarea {
        width: 100%;
        box-sizing: border-box;
        background: var(--vscode-input-background);
        color: var(--vscode-input-foreground);
        border: 1px solid var(--vscode-input-border, transparent);
        padding: 6px 8px;
        border-radius: 4px;
        font-family: inherit;
        font-size: 12px;
    }
    textarea {
        resize: vertical;
        min-height: 80px;
    }
    button {
        background: var(--vscode-button-background);
        color: var(--vscode-button-foreground);
        border: none;
        padding: 7px 12px;
        border-radius: 4px;
        cursor: pointer;
        font-weight: 600;
        font-size: 12px;
    }
    button:hover {
        background: var(--vscode-button-hoverBackground);
    }
    button.secondary {
        background: var(--vscode-button-secondaryBackground);
        color: var(--vscode-button-secondaryForeground);
        white-space: nowrap;
    }
    button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    #logs {
        background: var(--vscode-editor-background);
        border: 1px solid var(--vscode-panel-border);
        border-radius: 4px;
        padding: 8px;
        font-family: var(--vscode-editor-font-family, monospace);
        font-size: 11px;
        max-height: 220px;
        overflow-y: auto;
        white-space: pre-wrap;
        display: none;
    }
    .badge-ok { color: #4ec9b0; font-weight: bold; }
    .badge-err { color: #f14c4c; font-weight: bold; }
    .file-link {
        color: var(--vscode-textLink-foreground);
        cursor: pointer;
        text-decoration: underline;
        display: block;
        margin-top: 4px;
    }
</style>
</head>
<body>
    <label>Target Files (comma-separated)</label>
    <div class="row">
        <input id="targetFiles" type="text" placeholder="e.g. src/config.rs" />
        <button class="secondary" id="btnActive" title="Use active editor file">+ Active</button>
    </div>

    <label>Autonomous Task for Gemma 4</label>
    <textarea id="prompt" placeholder="Describe the Rust feature, function, or refactor to implement..."></textarea>

    <button id="btnRun">Run Astra Agent (AST + Clippy + Test)</button>

    <div id="logs"></div>
    <div id="output"></div>

    <script>
        const vscode = acquireVsCodeApi();
        const targetFilesEl = document.getElementById('targetFiles');
        const promptEl = document.getElementById('prompt');
        const btnRun = document.getElementById('btnRun');
        const btnActive = document.getElementById('btnActive');
        const logsEl = document.getElementById('logs');
        const outputEl = document.getElementById('output');

        btnActive.addEventListener('click', () => {
            vscode.postMessage({ type: 'useActiveFile' });
        });

        btnRun.addEventListener('click', () => {
            const prompt = promptEl.value.trim();
            if (!prompt) return;
            logsEl.style.display = 'block';
            logsEl.textContent = '';
            outputEl.innerHTML = '';
            vscode.postMessage({
                type: 'runTask',
                prompt,
                targetFiles: targetFilesEl.value
            });
        });

        window.addEventListener('message', (event) => {
            const msg = event.data;
            if (msg.type === 'activeFile') {
                const cur = targetFilesEl.value.trim();
                targetFilesEl.value = cur ? cur + ', ' + msg.file : msg.file;
            } else if (msg.type === 'status') {
                btnRun.disabled = msg.running;
                btnRun.textContent = msg.running ? 'Astra Working (Clippy & Tests)...' : 'Run Astra Agent (AST + Clippy + Test)';
            } else if (msg.type === 'log') {
                logsEl.style.display = 'block';
                logsEl.textContent += msg.text + '\\n';
                logsEl.scrollTop = logsEl.scrollHeight;
            } else if (msg.type === 'result') {
                btnRun.disabled = false;
                btnRun.textContent = 'Run Astra Agent (AST + Clippy + Test)';
                const res = msg.result;
                if (res.success) {
                    let html = '<div class="badge-ok">✓ Verified by clippy & cargo test (Attempts: ' + res.attempts_used + ')</div>';
                    for (const f of res.modified_files) {
                        html += '<a class="file-link" data-file="' + f + '">📄 ' + f + '</a>';
                    }
                    outputEl.innerHTML = html;
                    document.querySelectorAll('.file-link').forEach(el => {
                        el.addEventListener('click', () => {
                            vscode.postMessage({ type: 'openFile', file: el.getAttribute('data-file') });
                        });
                    });
                } else {
                    outputEl.innerHTML = '<div class="badge-err">✗ Verification failed after ' + res.attempts_used + ' attempts (changes rolled back)</div><pre>' + (res.last_error || '') + '</pre>';
                }
            } else if (msg.type === 'error') {
                btnRun.disabled = false;
                btnRun.textContent = 'Run Astra Agent (AST + Clippy + Test)';
                outputEl.innerHTML = '<div class="badge-err">Error: ' + msg.message + '</div>';
            }
        });
    </script>
</body>
</html>`;
    }
}