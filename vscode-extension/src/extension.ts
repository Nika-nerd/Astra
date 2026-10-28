import * as vscode from 'vscode';
import * as path from 'path';
import { AstraBackendClient } from './client';
import { AstraChatViewProvider } from './webview';

let backendClient: AstraBackendClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    backendClient = new AstraBackendClient(context.extensionPath);

    const chatProvider = new AstraChatViewProvider(context.extensionUri, backendClient);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(AstraChatViewProvider.viewType, chatProvider)
    );

    const runTaskCmd = vscode.commands.registerCommand('astra.runTask', async () => {
        const prompt = await vscode.window.showInputBox({
            prompt: 'Astra Agent: Describe the Rust task to implement and verify',
            placeHolder: 'e.g., Add a method to parse config timeouts with unit tests'
        });

        if (!prompt) {
            return;
        }

        let activeRel = '';
           const explicitFiles: string[] = [];
        const editor = vscode.window.activeTextEditor;
        const root = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (editor && root) {
            explicitFiles.push(path.relative(root, editor.document.uri.fsPath));
        }

        await chatProvider.handleRunTask(prompt, explicitFiles);
    });

    context.subscriptions.push(runTaskCmd);
}
export function deactivate(): void {
    if (backendClient) {
        backendClient.dispose();
    }
}