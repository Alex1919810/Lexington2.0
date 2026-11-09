// Development-only boundary checks. Node is provided by VS Code, not used by Lexington.
const assert = require('assert/strict');
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { EventEmitter } = require('events');
const root = path.join(__dirname, '..');
const commands = {};
const messages = [];
let errors = [];
let configuredExecutable = '/tmp/带 空格/lexington';
const vscode = {
    workspace: { getConfiguration: () => ({ get: () => configuredExecutable }), getWorkspaceFolder: () => undefined, textDocuments: [] },
    languages: { getDiagnostics: () => [] },
    window: { showErrorMessage: message => errors.push(message) },
    commands: { registerCommand: (name, callback) => { commands[name] = callback; return {}; } }
};
let invocation;
const spawn = (executable, args, options) => {
    invocation = { executable, args, options };
    const child = new EventEmitter();
    child.stderr = new EventEmitter();
    child.stdin = new EventEmitter();
    child.stdin.end = payload => messages.push(JSON.parse(payload));
    return child;
};
const exportsValue = {};
vm.runInNewContext(fs.readFileSync(path.join(root, 'integrations/vscode/extension.js'), 'utf8'), {
    require: name => name === 'vscode' ? vscode : name === 'child_process' ? { spawn } : require(name),
    module: exportsValue, process, Buffer
});
exportsValue.exports.activate({ subscriptions: [] });
const localFile = { scheme: 'file', fsPath: '/tmp/引号 " $(不可执行).rs', toString: () => 'file:///tmp/example.rs' };
vscode.window.activeTextEditor = {
    selections: [{ isEmpty: false, start: { line: 8 } }],
    document: { uri: localFile, isUntitled: false, getText: () => '第一行\n第二行' }
};
commands['lexington.askSelection']();
assert.equal(invocation.executable, '/tmp/带 空格/lexington');
assert.deepEqual(Array.from(invocation.args), ['--ask-json']);
assert.equal(invocation.options.shell, undefined);
assert.deepEqual(messages.pop(), { text: '第一行\n第二行', files: [localFile.fsPath], source: 'file:///tmp/example.rs', line: 9 });
vscode.window.activeTextEditor.document.uri = { scheme: 'vscode-remote', toString: () => 'vscode-remote://ssh-remote+dev/home/code.rs' };
commands['lexington.askSelection']();
const remote = messages.pop();
assert.deepEqual(remote.files, []);
assert.match(remote.source, /vscode-remote/);
vscode.window.activeTextEditor.document.isUntitled = true;
commands['lexington.askSelection']();
assert.match(messages.pop().source, /未保存/);
commands['lexington.askFile'](localFile, [localFile]);
assert.deepEqual(messages.pop().files, [localFile.fsPath]);
commands['lexington.askFile']({ scheme: 'vscode-remote' });
assert.equal(errors.length, 1);
configuredExecutable = '';
commands['lexington.askFile'](localFile, [localFile]);
assert.equal(invocation.options.windowsHide, true);
if (process.platform === 'win32') {
    assert.equal(invocation.executable, path.join(process.env.LOCALAPPDATA || path.join(require('os').homedir(), 'AppData', 'Local'), 'lexington', 'lexington.exe'));
}
messages.pop();

const workspace = { uri: { scheme: 'file', fsPath: '/tmp/project', toString: () => 'file:///tmp/project' } };
vscode.workspace.getWorkspaceFolder = () => workspace;
vscode.window.activeTextEditor.document.uri = localFile;
vscode.window.activeTextEditor.document.isUntitled = false;
commands['lexington.askSelection']();
assert.equal(messages.pop().workspace, workspace.uri.fsPath);
commands['lexington.askFile'](localFile, [localFile]);
assert.equal(messages.pop().workspace, workspace.uri.fsPath);
vscode.workspace.getWorkspaceFolder = file => file === localFile ? workspace : undefined;
const previousErrors = errors.length;
commands['lexington.askFile'](localFile, [localFile, { scheme: 'file', fsPath: '/tmp/other.rs' }]);
assert.equal(errors.length, previousErrors + 1);
assert.match(errors.at(-1), /不同项目/);

const issue = { severity: 0, message: '类型不匹配', source: 'rust-analyzer', code: { value: 'E0308' },
    range: { start: { line: 3, character: 2 }, end: { line: 3, character: 5 } } };
vscode.workspace.getWorkspaceFolder = uri => uri === localFile ? workspace : undefined;
vscode.workspace.textDocuments = [{ uri: localFile, version: 7, isDirty: true }];
vscode.languages.getDiagnostics = () => [[localFile, [issue]], [{ scheme: 'file', fsPath: '/elsewhere/a.rs', toString: () => 'file:///elsewhere/a.rs' }, [issue]]];
commands['lexington.sendDiagnostics'](localFile);
const diagnosticRequest = messages.pop();
assert.equal(diagnosticRequest.text, '');assert.equal(diagnosticRequest.workspace, workspace.uri.fsPath);
assert.equal(diagnosticRequest.diagnostics.documents.length, 1);
assert.equal(diagnosticRequest.diagnostics.documents[0].version, 7);
assert.equal(diagnosticRequest.diagnostics.documents[0].dirty, true);
assert.deepEqual(diagnosticRequest.diagnostics.documents[0].diagnostics[0], {
    severity: 'error', message: '类型不匹配', source: 'rust-analyzer', code: 'E0308', startLine: 4, startColumn: 3, endLine: 4, endColumn: 6
});
vscode.languages.getDiagnostics = () => [[localFile, Array.from({ length: 210 }, () => issue)]];
commands['lexington.sendDiagnostics'](localFile);
const capped = messages.pop();assert.equal(capped.diagnostics.truncated, true);assert.equal(capped.diagnostics.documents[0].diagnostics.length, 200);
vscode.languages.getDiagnostics = () => [[localFile, Array.from({ length: 200 }, () => ({ ...issue, message: '长'.repeat(4000) }))]];
commands['lexington.sendDiagnostics'](localFile);const byteCapped = messages.pop();assert.ok(byteCapped.diagnostics.truncated);assert.ok(Buffer.byteLength(JSON.stringify(byteCapped.diagnostics)) < 512 * 1024);
vscode.languages.getDiagnostics = () => [[localFile, [{ ...issue, message: 'x'.repeat(3999) + '🚨', source: '源'.repeat(128) }]]];
commands['lexington.sendDiagnostics'](localFile);const unicodeBoundary = messages.pop().diagnostics.documents[0].diagnostics[0];assert.equal(unicodeBoundary.message, 'x'.repeat(3999));assert.equal(unicodeBoundary.source, '源'.repeat(128));
vscode.languages.getDiagnostics = () => [[localFile, [{ ...issue, code: null }]]];commands['lexington.sendDiagnostics'](localFile);assert.equal(messages.pop().diagnostics.documents[0].diagnostics[0].code, '');
vscode.languages.getDiagnostics = () => [];
commands['lexington.sendDiagnostics'](localFile);assert.deepEqual(messages.pop().diagnostics.documents, []);
commands['lexington.askSelection']();assert.ok(messages.pop().diagnostics);

let installed;
let clicked;
let menu;
let badge;
let nativeResponse = { ok: true };
const chrome = {
    runtime: {
        onInstalled: { addListener: callback => { installed = callback; } },
        sendNativeMessage: (host, request, callback) => {
            assert.equal(host, 'com.lexington.context');
            messages.push(JSON.parse(JSON.stringify(request)));
            callback(nativeResponse);
        }
    },
    contextMenus: {
        removeAll: callback => callback(), create: value => { menu = value; },
        onClicked: { addListener: callback => { clicked = callback; } }
    },
    action: { setBadgeText: value => { badge = value.text; }, setTitle: () => {} }
};
vm.runInNewContext(fs.readFileSync(path.join(root, 'integrations/browser/background.js'), 'utf8'), { chrome, URL, TextEncoder });
installed();
assert.equal(menu.title, '看看Lexington怎么说');
clicked({ menuItemId: menu.id, selectionText: '网页选区', frameUrl: 'https://example.com/frame', pageUrl: 'https://example.com/' });
assert.deepEqual(messages.pop(), { text: '网页选区', files: [], source: 'https://example.com/frame' });
clicked({ menuItemId: menu.id, selectionText: '本地选区', pageUrl: 'file:///tmp/%E4%B8%AD%E6%96%87%20%22.rs' });
assert.deepEqual(messages.pop().files, ['/tmp/中文 ".rs']);
nativeResponse = { ok: false, error: '队列已满' };
clicked({ menuItemId: menu.id, selectionText: 'Windows 文件', pageUrl: 'file:///C:/Users/%E4%B8%AD%E6%96%87/a%20b.rs' });
assert.deepEqual(messages.pop().files, ['C:\\Users\\中文\\a b.rs']);
clicked({ menuItemId: menu.id, selectionText: '失败提示', pageUrl: 'https://example.com/' });
assert.equal(badge, '!');
console.log('VS Code and browser context-menu boundary checks passed.');
