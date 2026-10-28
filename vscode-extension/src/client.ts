import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import * as vscode from 'vscode';



export interface FilePatch {
    relative_path: string;
    content: string;
}

export interface AgentTask {
    prompt: string;
    target_files: string[];
    workspace_root?: string;
}

export interface AgentResult {
    success: boolean;
    attempts_used: number;
    explanation: string;
    proposed_patches: FilePatch[];
    modified_files: string[];
    last_error: string | null;
}


interface RpcResponse {
    id: string;
    result: AgentResult | null;
    error: string | null;
}

export class AstraBackendClient {
    private childProc: cp.ChildProcessWithoutNullStreams | null = null;
    private outputChannel: vscode.OutputChannel;
    private pendingRequests = new Map<string, {
        resolve: (res: AgentResult) => void;
        reject: (err: Error) => void;
    }>();
    private stdoutBuffer = '';
    private onLogCallback?: (logLine: string) => void;

    constructor(private extensionRoot: string) {
        this.outputChannel = vscode.window.createOutputChannel('Astra Agent');
    }

    public onLog(cb: (logLine: string) => void) {
        this.onLogCallback = cb;
    }

    private resolveBinaryPath(): { cmd: string; args: string[]; cwd: string } {
        const config = vscode.workspace.getConfiguration('astra');
        const customPath = config.get<string>('backendPath');
        if (customPath && fs.existsSync(customPath)) {
            return { cmd: customPath, args: [], cwd: path.dirname(customPath) };
        }

        const repoRoot = path.resolve(this.extensionRoot, '..');
        const releaseBin = path.join(repoRoot, 'target', 'release', 'astra-backend');
        const debugBin = path.join(repoRoot, 'target', 'debug', 'astra-backend');

        if (fs.existsSync(releaseBin)) {
            return { cmd: releaseBin, args: [], cwd: repoRoot };
        }
        if (fs.existsSync(debugBin)) {
            return { cmd: debugBin, args: [], cwd: repoRoot };
        }

        return { cmd: 'cargo', args: ['run', '--quiet', '--'], cwd: repoRoot };
    }

    private ensureStarted(workspaceRoot: string) {
        if (this.childProc && !this.childProc.killed) {
            return;
        }

        const { cmd, args, cwd } = this.resolveBinaryPath();
        const config = vscode.workspace.getConfiguration('astra');
        const model = config.get<string>('llmModel') || 'gemma4';

        this.outputChannel.appendLine(`[Astra] Starting backend: ${cmd} ${args.join(' ')}`);

        this.childProc = cp.spawn(cmd, args, {
            cwd,
            env: {
                ...process.env,
                ASTRA_LLM_MODEL: model,
                ASTRA_WORKSPACE: workspaceRoot,
                RUST_LOG: 'info'
            }
        });

        this.childProc.stdout.on('data', (data: Buffer) => {
            this.stdoutBuffer += data.toString();
            let newlineIdx: number;
            while ((newlineIdx = this.stdoutBuffer.indexOf('\n')) !== -1) {
                const line = this.stdoutBuffer.slice(0, newlineIdx).trim();
                this.stdoutBuffer = this.stdoutBuffer.slice(newlineIdx + 1);
                if (line) {
                    this.handleStdoutLine(line);
                }
            }
        });

       this.childProc.stderr.on('data', (data: Buffer) => {
            const text = data.toString();
            this.outputChannel.append(text);
            if (this.onLogCallback) {
                for (const line of text.split('\n')) {
                    const trimmed = line.trim();
                    if (trimmed) {
                        this.onLogCallback(trimmed);
                    }
                }
            }
        });

        this.childProc.on('error', (err: Error) => {
            this.outputChannel.appendLine(`[Astra] Process error: ${err.message}`);
        });

        this.childProc.stdin.on('error', (err: Error) => {
            this.outputChannel.appendLine(`[Astra] Stdin error: ${err.message}`);
        });

        this.childProc.on('exit', (code: number | null) => {
            this.outputChannel.appendLine(`[Astra] Backend exited with code ${code}`);
            this.childProc = null;
            for (const [, req] of this.pendingRequests) {
                req.reject(new Error(`Astra backend process exited unexpectedly (code ${code})`));
            }
            this.pendingRequests.clear();
        });
    }

    private handleStdoutLine(line: string) {
        try {
            const resp: RpcResponse = JSON.parse(line);
            const pending = this.pendingRequests.get(resp.id);
            if (!pending) {
                return;
            }
            this.pendingRequests.delete(resp.id);

            if (resp.error) {
                pending.reject(new Error(resp.error));
            } else if (resp.result) {
                pending.resolve(resp.result);
            } else {
                pending.reject(new Error('Empty response from Astra backend'));
            }
        } catch (err) {
            this.outputChannel.appendLine(`[Astra] Failed to parse stdout JSON: ${line}`);
        }
    }

    public async executeTask(task: AgentTask): Promise<AgentResult> {
        const workspaceRoot = task.workspace_root || vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '.';
        this.ensureStarted(workspaceRoot);

        const id = `task-${Date.now()}`;
        const payload = JSON.stringify({ id, task: { ...task, workspace_root: workspaceRoot } }) + '\n';

        return new Promise<AgentResult>((resolve, reject) => {
            this.pendingRequests.set(id, { resolve, reject });
            this.childProc!.stdin.write(payload, (err?: Error | null) => {
                if (err) {
                    this.pendingRequests.delete(id);
                    reject(err);
                }
            });
        });
    }

    public dispose() {
        if (this.childProc) {
            this.childProc.kill();
            this.childProc = null;
        }
        this.outputChannel.dispose();
    }
}