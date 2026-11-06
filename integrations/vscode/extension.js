const vscode = require('vscode');
const { spawn } = require('child_process');
const path = require('path');
const os = require('os');

function send(request) {
    const configured = vscode.workspace.getConfiguration('lexington').get('executable');
    const data = process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share');
    const executable = configured || path.join(data, 'lexington', 'lexington');
    const payload = JSON.stringify(request);
    if (Buffer.byteLength(payload) > 1024 * 1024) {
        vscode.window.showErrorMessage('选中内容超过 1 MiB，请缩小选区。');
        return;
    }
    // No shell, command interpolation, clipboard modification, or temporary text files.
    const child = spawn(executable, ['--ask-json'], { stdio: ['pipe', 'ignore', 'pipe'] });
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

function activate(context) {
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
            line: Math.min(...selections.map(selection => selection.start.line)) + 1
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
        send({ text: '', files: files.map(file => file.fsPath), source: 'VS Code 文件资源管理器' });
    }));
}

module.exports = { activate };
