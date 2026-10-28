import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { AstraBackendClient, FilePatch } from './client';
import { getChatHtml } from './chatHtml';

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

        webviewView.webview.html = getChatHtml();

        webviewView.webview.onDidReceiveMessage(async (data: {
            type: string;
            prompt?: string;
            targetFiles?: string[];
            patches?: FilePatch[];
            messageId?: string;
        }) => {
            if (data.type === 'runTask' && data.prompt) {
                await this.handleRunTask(data.prompt, data.targetFiles || []);
            } else if (data.type === 'requestFileList') {
                await this.sendWorkspaceFiles();
            } else if (data.type === 'applyPatches' && data.patches) {
                await this.applyPatchesToWorkspace(data.patches, data.messageId);
            }
        });
    }

    private getActiveWorkspaceRoot(): string | undefined {
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor) {
            const folder = vscode.workspace.getWorkspaceFolder(activeEditor.document.uri);
            if (folder) {
                return folder.uri.fsPath;
            }
        }
        return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    }

    private async sendWorkspaceFiles(): Promise<void> {
        const root = this.getActiveWorkspaceRoot();
        if (!root) {
            this.postMessage({ type: 'fileList', files: [] });
            return;
        }

        const uris = await vscode.workspace.findFiles(
            '**/*.{rs,toml}',
            '{**/target/**,**/node_modules/**,**/.git/**}',
            200
        );

        const files = uris
            .map((u) => path.relative(root, u.fsPath))
            .filter((rel) => !rel.startsWith('..'))
            .sort();

        this.postMessage({ type: 'fileList', files, workspaceName: path.basename(root) });
    }

    private async applyPatchesToWorkspace(patches: FilePatch[], messageId?: string): Promise<void> {
        const root = this.getActiveWorkspaceRoot();
        if (!root) {
            vscode.window.showErrorMessage('Astra: No workspace folder open to apply changes.');
            return;
        }

        try {
            for (const patch of patches) {
                const fullPath = path.join(root, patch.relative_path);
                const dir = path.dirname(fullPath);
                if (!fs.existsSync(dir)) {
                    fs.mkdirSync(dir, { recursive: true });
                }
                fs.writeFileSync(fullPath, patch.content, 'utf8');

                const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fullPath));
                await vscode.window.showTextDocument(doc, { preview: false });
            }

            vscode.window.showInformationMessage(
                `Astra: Applied ${patches.length} verified file(s) to project.`
            );
            this.postMessage({ type: 'applied', messageId });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            vscode.window.showErrorMessage(`Astra failed to apply changes: ${msg}`);
        }
    }

    public async handleRunTask(rawPrompt: string, explicitFiles: string[] = []): Promise<void> {
        const root = this.getActiveWorkspaceRoot();
        if (!root) {
            this.postMessage({
                type: 'error',
                message: 'Please open a Rust project folder (File -> Open Folder) so Astra can read its AST.'
            });
            return;
        }

        const mentionRegex = /@([\w./-]+\.(?:rs|toml))/g;
        const mentionedFiles = new Set<string>(explicitFiles);
        let match: RegExpExecArray | null;
        while ((match = mentionRegex.exec(rawPrompt)) !== null) {
            mentionedFiles.add(match[1]);
        }

        if (mentionedFiles.size === 0 && vscode.window.activeTextEditor) {
            const activePath = vscode.window.activeTextEditor.document.uri.fsPath;
            if (activePath.endsWith('.rs')) {
                const rel = path.relative(root, activePath);
                if (!rel.startsWith('..')) {
                    mentionedFiles.add(rel);
                }
            }
        }

        const target_files = Array.from(mentionedFiles);

        this.postMessage({
            type: 'userMessage',
            text: rawPrompt,
            targetFiles: target_files,
            workspaceName: path.basename(root)
        });

        try {
            const result = await this.client.executeTask({
                prompt: rawPrompt,
                target_files,
                workspace_root: root
            });
            this.postMessage({ type: 'agentResult', result });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            this.postMessage({ type: 'error', message: msg });
        }
    }

    private postMessage(msg: unknown): void {
        this.view?.webview.postMessage(msg);
    }
}