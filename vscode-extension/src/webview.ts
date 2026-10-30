import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { AstraBackendClient } from './client';
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
            previousPlan?: string;
            approvedPlan?: string;
            originalPrompt?: string;
            file?: string;
        }) => {
            if (data.type === 'runTask' && data.prompt) {
                await this.handleRunTask(data.prompt, data.targetFiles || [], data.previousPlan);
            } else if (data.type === 'executePlan' && data.approvedPlan) {
                await this.handleExecutePlan(data.originalPrompt || '', data.approvedPlan, data.targetFiles || []);
            } else if (data.type === 'requestFileList') {
                await this.sendWorkspaceFiles();
            } else if (data.type === 'openFile' && data.file) {
                await this.openWorkspaceFile(data.file);
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

    private async openWorkspaceFile(relPath: string): Promise<void> {
        const root = this.getActiveWorkspaceRoot();
        if (!root) {
            return;
        }
        const fullPath = path.join(root, relPath);
        if (fs.existsSync(fullPath)) {
            const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fullPath));
            await vscode.window.showTextDocument(doc, { preview: false });
        }
    }

    public async handleRunTask(
        rawPrompt: string,
        explicitFiles: string[] = [],
        previousPlan?: string
    ): Promise<void> {
        const root = this.getActiveWorkspaceRoot();
        if (!root) {
            this.postMessage({
                type: 'error',
                message: 'Please open a folder (File -> Open Folder) so Astra can read its AST and build your project.'
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
            workspaceName: path.basename(root),
            isRefinement: Boolean(previousPlan)
        });

        try {
            const result = await this.client.executeTask({
                mode: 'plan',
                prompt: rawPrompt,
                target_files,
                workspace_root: root,
                previous_plan: previousPlan
            });
            this.postMessage({
                type: 'planResult',
                result,
                originalPrompt: rawPrompt,
                targetFiles: target_files
            });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            this.postMessage({ type: 'error', message: msg });
        }
    }

    public async handleExecutePlan(
        originalPrompt: string,
        approvedPlan: string,
        targetFiles: string[]
    ): Promise<void> {
        const root = this.getActiveWorkspaceRoot();
        if (!root) {
            return;
        }

        try {
            const result = await this.client.executeTask({
                mode: 'execute',
                prompt: originalPrompt,
                target_files: targetFiles,
                workspace_root: root,
                approved_plan: approvedPlan
            });

            this.postMessage({ type: 'buildResult', result });

            if (result.success && result.modified_files.length > 0) {
                await this.openWorkspaceFile(result.modified_files[0]);
                await this.sendWorkspaceFiles();
            }
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            this.postMessage({ type: 'error', message: msg });
        }
    }

    private postMessage(msg: unknown): void {
        this.view?.webview.postMessage(msg);
    }
}