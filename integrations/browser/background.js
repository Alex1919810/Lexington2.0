const menuId = 'lexington-ask';
const title = '看看Lexington怎么说';
chrome.runtime.onInstalled.addListener(() => {
    chrome.contextMenus.removeAll(() => {
        chrome.contextMenus.create({ id: menuId, title, contexts: ['selection'] });
    });
});
chrome.contextMenus.onClicked.addListener(info => {
    if (info.menuItemId !== menuId || !info.selectionText) return;
    const source = info.frameUrl || info.pageUrl || '';
    const request = { text: info.selectionText, files: [], source };
    try {
        const url = new URL(source);
        if (url.protocol === 'file:' && (!url.hostname || url.hostname === 'localhost')) {
            const pathname = decodeURIComponent(url.pathname);
            request.files = [/^\/[a-z]:\//i.test(pathname) ? pathname.slice(1).replace(/\//g, '\\') : pathname];
        }
    } catch (_) { /* Preserve the original source when no local path is available. */ }
    const action = chrome.action || chrome.browserAction;
    if (new TextEncoder().encode(JSON.stringify(request)).length > 1024 * 1024) {
        action.setBadgeText({ text: '!' });
        action.setTitle({ title: '选中内容超过 1 MiB，请缩小选区' });
        return;
    }
    chrome.runtime.sendNativeMessage('com.lexington.context', request, response => {
        const error = chrome.runtime.lastError;
        const message = error ? error.message : (!response || !response.ok ? (response && response.error) || '没有收到确认' : '');
        action.setBadgeText({ text: message ? '!' : '' });
        action.setTitle({ title: message ? `发送失败：${message}` : title });
    });
});
