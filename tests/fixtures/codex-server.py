"""Deterministic app-server peer; never starts a model or accesses user credentials."""
import json
import os
import subprocess
import sys

mode = sys.argv[1]
thread = "fixture-thread"
count = 0
pending = None
turn = None
deferred_ack = None


def send(value):
    print(json.dumps(value, ensure_ascii=False), flush=True)


def notify(method, params):
    send({"method": method, "params": {"threadId": thread, "turnId": turn, **params}})


def complete(text, status="completed"):
    item = {"type": "agentMessage", "id": "reply-" + turn, "text": text}
    notify("item/completed", {"item": item})
    notify("turn/completed", {"turn": {"id": turn, "status": status, "items": [item]}})


for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    params = message.get("params", {})
    if method == "initialize":
        assert message["params"]["clientInfo"]["name"] == "lexington"
        if mode == "startup-error":
            send({"id": message["id"], "error": {"code": -32000, "message": "请先登录 Codex"}})
        else:
            send({"id": message["id"], "result": {"userAgent": "mock"}})
    elif method == "initialized":
        pass
    elif method == "thread/start":
        assert os.path.isabs(params["cwd"])
        assert params["cwd"] == os.getcwd()
        send({"id": message["id"], "result": {"thread": {"id": thread}}})
    elif method == "turn/start":
        assert params["threadId"] == thread
        if deferred_ack is not None:
            send(deferred_ack)
            deferred_ack = None
        count += 1
        turn = f"turn-{count}"
        text = params["input"][0]["text"]
        if mode == "crash":
            sys.exit(3)
        if mode == "error" or text == "fail":
            send({"id": message["id"], "error": {"code": -32000, "message": "模拟请求失败"}})
            continue
        if text not in ("fast", "late-ack"):
            send({"id": message["id"], "result": {"turn": {"id": turn}}})
        if text == "descendant":
            child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
            complete(str(child.pid))
        elif text == "approve":
            pending = "approve"
            send({"id": "approval-1", "method": "item/commandExecution/requestApproval", "params": {"threadId": thread, "turnId": turn, "reason": "运行项目检查", "command": "cargo test", "cwd": os.getcwd()}})
        elif text == "file-approval":
            pending = "file-approval"
            notify("item/started", {"item": {"type": "fileChange", "id": "patch-1", "changes": [{"path": "example.txt", "kind": {"type": "update"}, "diff": "-before\n+after"}]}})
            send({"id": "approval-file", "method": "item/fileChange/requestApproval", "params": {"threadId": thread, "turnId": turn, "itemId": "patch-1", "reason": "修改示例文件"}})
        elif text == "question":
            pending = "question"
            send({"id": 41, "method": "item/tool/requestUserInput", "params": {"threadId": thread, "turnId": turn, "questions": [{"id": "choice", "question": "选择方案", "options": [{"label": "A"}, {"label": "B"}]}]}})
        elif text == "unsupported":
            pending = "unsupported"
            send({"id": 42, "method": "item/tool/call", "params": {}})
        else:
            send({"method": "item/agentMessage/delta", "params": {"threadId": "unrelated", "turnId": turn, "itemId": "noise", "delta": "不应显示"}})
            prefix = f"第{count}轮："
            notify("item/agentMessage/delta", {"itemId": "reply-" + turn, "delta": prefix})
            notify("item/agentMessage/delta", {"itemId": "reply-" + turn, "delta": text})
            if text != "interrupt":
                complete(prefix + text)
                if text == "fast":
                    send({"id": message["id"], "result": {"turn": {"id": turn}}})
                elif text == "late-ack":
                    deferred_ack = {"id": message["id"], "result": {"turn": {"id": turn}}}
    elif method == "turn/interrupt":
        assert params["turnId"] == turn
        send({"id": message["id"], "result": {}})
        complete("已停止", "interrupted")
        pending = None
    elif method is None:
        if pending == "approve":
            assert message["id"] == "approval-1"
            assert message["result"]["decision"] == "decline"
            complete("已拒绝命令")
        elif pending == "file-approval":
            assert message["id"] == "approval-file"
            assert message["result"]["decision"] == "decline"
            complete("已拒绝文件改动")
        elif pending == "question":
            assert message["result"]["answers"]["choice"]["answers"] == ["A"]
            complete("收到选择 A")
        elif pending == "unsupported":
            assert message["error"]["code"] == -32601
            complete("不支持的请求已明确返回错误")
        else:
            raise AssertionError(message)
        pending = None
    else:
        raise AssertionError(message)
