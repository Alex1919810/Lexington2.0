const vscode = require('vscode');
const { spawn } = require('child_process');
const path = require('path');
const os = require('os');

function send(request) {
    const configured = vscode.workspace.getConfiguration('lexington').get('executable');
    const windows = process.platform === 'win32';
    const data = windows ? process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local')
        : process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share');
    const executable = configured || path.join(data, 'lexington', windows ? 'lexington.exe' : 'lexington');
    const payload = JSON.stringify(request);
    if (Buffer.byteLength(payload) > 1024 * 1024) {
        vscode.window.showErrorMessage('选中内容超过 1 MiB，请缩小选区。');
        return;
    }
    // No shell, command interpolation, clipboard modification, or temporary text files.
    const child = spawn(executable, ['--ask-json'], { stdio: ['pipe', 'ignore', 'pipe'], windowsHide: true });
    let error = '';
    child.stderr.on('data', chunk => { error = (error + chunk.toString()).slice(-4096); });
    child.on('error', problem => vscode.window.showErrorMessage(`无法发送给 Lexington：${problem.message}`));
    child.on('close', code => {
        if (code !== null && code !== 0) {
            vscode.window.showErrorMessage(`无法发送给 Lexington：${error.trim() || '程序退出'}`);
        }
    });
    child.stdin.on('error', () => {}); // A failed spawn/early exit is reported above.
    child.stdin.end(payload);
}

function limitText(value, maximum) {
    const text = String(value).slice(0, maximum);
    const last = text.charCodeAt(text.length - 1);
    return last >= 0xd800 && last <= 0xdbff ? text.slice(0, -1) : text;
}

function diagnostics(workspace) {
    if (workspace?.uri.scheme !== 'file') return {};
    const snapshot = { capturedAt: Math.floor(Date.now() / 1000), truncated: false, documents: [] };
    let total = 0;
    let bytes = 128;
    for (const [uri, issues] of vscode.languages.getDiagnostics()) {
        if (uri.scheme !== 'file' || vscode.workspace.getWorkspaceFolder(uri)?.uri.toString() !== workspace.uri.toString()) continue;
        if (snapshot.documents.length >= 100) { snapshot.truncated = true; break; }
        const document = vscode.workspace.textDocuments.find(item => item.uri.toString() === uri.toString());
        const selected = [];
        const metadataBytes = Buffer.byteLength(JSON.stringify({ uri: uri.toString(), path: uri.fsPath })) + 128;
        if (bytes + metadataBytes > 512 * 1024) { snapshot.truncated = true; break; }
        bytes += metadataBytes;
        for (const issue of issues) {
            if (total >= 200) { snapshot.truncated = true; break; }
            const item = {
                severity: ['error', 'warning', 'information', 'hint'][issue.severity] || 'information',
                message: limitText(issue.message, 4000), source: limitText(issue.source || '', 256),
                code: limitText(issue.code && typeof issue.code === 'object' ? issue.code.value : issue.code ?? '', 256),
                startLine: issue.range.start.line + 1, startColumn: issue.range.start.character + 1,
                endLine: issue.range.end.line + 1, endColumn: issue.range.end.character + 1
            };
            const itemBytes = Buffer.byteLength(JSON.stringify(item)) + 2;
            if (bytes + itemBytes > 512 * 1024) { snapshot.truncated = true; break; }
            bytes += itemBytes; selected.push(item); total++;
        }
        snapshot.documents.push({ uri: uri.toString(), path: uri.fsPath, version: document?.version ?? null,
            dirty: document?.isDirty || false, diagnostics: selected });
    }
    return { diagnostics: snapshot };
}

function activate(context) {
    context.subscriptions.push(vscode.commands.registerCommand('lexington.sendDiagnostics', uri => {
        const resource = uri || vscode.window.activeTextEditor?.document.uri;
        const workspace = resource ? vscode.workspace.getWorkspaceFolder(resource) : vscode.workspace.workspaceFolders?.length === 1 ? vscode.workspace.workspaceFolders[0] : undefined;
        if (workspace?.uri.scheme !== 'file') {
            vscode.window.showErrorMessage('请在目标本地项目中打开文件，再发送项目诊断。');
            return;
        }
        send({ text: '', files: [], source: 'VS Code 项目诊断', workspace: workspace.uri.fsPath, ...diagnostics(workspace) });
    }));
    context.subscriptions.push(vscode.commands.registerCommand('lexington.askSelection', () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return;
        const selections = editor.selections.filter(selection => !selection.isEmpty);
        if (!selections.length) return;
        const document = editor.document;
        send({
            text: selections.map(selection => document.getText(selection)).join('\n\n'),
            files: document.uri.scheme === 'file' ? [document.uri.fsPath] : [],
            source: document.isUntitled ? 'VS Code：未保存文档（无磁盘路径）' : document.uri.toString(),
            line: Math.min(...selections.map(selection => selection.start.line)) + 1,
            ...(document.uri.scheme === 'file' && vscode.workspace.getWorkspaceFolder(document.uri)?.uri.scheme === 'file'
                ? { workspace: vscode.workspace.getWorkspaceFolder(document.uri).uri.fsPath, ...diagnostics(vscode.workspace.getWorkspaceFolder(document.uri)) } : {})
        });
    }));
    context.subscriptions.push(vscode.commands.registerCommand('lexington.askFile', (uri, selected) => {
        const files = selected && selected.length ? selected : [uri];
        if (files.some(file => !file || file.scheme !== 'file')) {
            vscode.window.showErrorMessage('请选择本地文件；远程文档请选中文字后使用。');
            return;
        }
        if (files.length > 32) {
            vscode.window.showErrorMessage('一次最多选择 32 个文件。');
            return;
        }
        const folders = files.map(file => vscode.workspace.getWorkspaceFolder(file));
        const workspace = folders[0];
        if (folders.some(folder => folder?.uri.toString() !== workspace?.uri.toString())) {
            vscode.window.showErrorMessage('文件属于不同项目，请分项目发送。');
            return;
        }
        send({ text: '', files: files.map(file => file.fsPath), source: 'VS Code 文件资源管理器',
            ...(workspace?.uri.scheme === 'file' ? { workspace: workspace.uri.fsPath, ...diagnostics(workspace) } : {}) });
    }));
}

module.exports = { activate };
