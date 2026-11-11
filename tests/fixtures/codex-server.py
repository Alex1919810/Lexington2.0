"""Deterministic app-server peer; never starts a model or accesses user credentials."""
import json
import os
import subprocess
import sys

mode = sys.argv[1]
if mode in ('api-config', 'inherit-config'):
    args = sys.argv[2:]
    assert args[:2] == ['app-server', '--stdio']
    overrides = {}
    for index in range(2, len(args), 2):
        assert args[index] == '-c'
        key, value = args[index + 1].split('=', 1)
        overrides[key] = json.loads(value)
    assert overrides['model'] == 'fixture-model"with-quote'
    if mode == 'api-config':
        assert overrides['model_provider'] == 'lexington_api'
        assert overrides['model_providers.lexington_api.base_url'] == 'http://127.0.0.1:9999/v1'
        assert overrides['model_providers.lexington_api.wire_api'] == 'responses'
        assert overrides['model_providers.lexington_api.env_key'] == 'LEXINGTON_MODEL_API_KEY'
        assert overrides['model_providers.lexington_api.requires_openai_auth'] is False
        assert os.environ.get('LEXINGTON_MODEL_API_KEY') == 'fixture-key-not-a-real-secret'
        assert not any('fixture-key-not-a-real-secret' in arg for arg in args)
    else:
        assert list(overrides) == ['model']
        assert 'LEXINGTON_MODEL_API_KEY' not in os.environ
thread = "fixture-thread"
count = 0
pending = None
turn = None
deferred_ack = None
steers = []
rate_limit_reads = 0
delayed_rate_limit = None


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
    elif method == "config/read" and mode.startswith("memory-"):
        send({"id":message["id"],"result":{"config":{"mcp_servers":{"fixture.with.dot":{"command":"fixture-only","required":True}}}}})
    elif method == "account/rateLimits/read":
        assert params is None, "quota reads must not advertise unsupported features"
        rate_limit_reads += 1
        if mode == "quota-error":
            send({"id": message["id"], "error": {"code": -32000, "message": "当前登录方式不支持账号额度查询"}})
        elif mode == "quota-late":
            delayed_rate_limit = message["id"]
        elif mode == "quota-empty":
            send({"id": message["id"], "result": {"rateLimits": {}, "rateLimitsByLimitId": None}})
        else:
            bucket = {"limitId": "codex", "planType": "plus", "primary": {"usedPercent": 25 if rate_limit_reads == 1 else 40, "windowDurationMins": 300, "resetsAt": 1800000000}, "secondary": {"usedPercent": 35 if rate_limit_reads == 1 else 50, "windowDurationMins": 10080, "resetsAt": 1800600000}}
            send({"id": message["id"], "result": {"rateLimits": bucket, "rateLimitsByLimitId": {"codex": bucket}}})
    elif method in ("thread/start", "thread/resume"):
        session_method = method
        if method == "thread/resume":
            assert params["threadId"] == thread
        assert os.path.isabs(params["cwd"])
        assert os.path.samefile(params["cwd"], os.getcwd())
        assert 'baseInstructions' not in params
        if mode.startswith("memory-"):
            assert params['ephemeral'] is True
            assert params['sandbox']=='read-only'
            assert params['approvalPolicy']=='never'
            assert params['config']['mcp_servers']['fixture.with.dot']=={'enabled':False,'required':False}
            assert params.get('developerInstructions')
        elif mode == "persona":
            assert params.get("developerInstructions") == "角色甲\n只属于甲"
        elif mode.startswith("development-"):
            instructions = params.get("developerInstructions", "")
            assert ("[Lexington development]" in instructions) == (mode == "development-on")
        elif mode == "schedule":
            assert method == "thread/start", "scheduled runs must use isolated fresh threads"
            if params.get("developerInstructions"):
                assert "[Lexington development]" in params["developerInstructions"]
        elif mode == "context":
            assert params.get("developerInstructions")
        else:
            assert "developerInstructions" not in params
        result = {"thread": {"id": thread}, "approvalPolicy": {"granular": {"sandbox_approval": True, "rules": False, "skill_approval": True, "request_permissions": True, "mcp_elicitations": True}} if mode == "granular" else "on-request"}
        if mode == "commands":
            result.update(model="fixture-model", modelProvider="fixture-provider", reasoningEffort="high", cwd=os.getcwd(), sandbox={"type": "workspaceWrite", "networkAccess": False})
        send({"id": message["id"], "result": result})
    elif method == "command/exec":
        assert os.path.samefile(params['cwd'], os.getcwd())
        assert 'sandboxPolicy' not in params and 'env' not in params, 'inherit local execution policy'
        assert params['outputBytesCap']==262144
        assert params['timeoutMs'] > 0
        assert params['command'][0]=='fixture-test'
        if params['command'][1]=='wait':
            pending=('standalone-test',message['id'],params['processId'])
        elif params['command'][1]=='error':
            send({'id':message['id'],'error':{'code':-32000,'message':'fixture execution error'}})
        else:
            send({'id':message['id'],'result':{'exitCode':1 if params['command'][1]=='fail' else 0,'stdout':json.dumps(params['command'],ensure_ascii=False),'stderr':'fixture failure' if params['command'][1]=='fail' else ''}})
    elif method == "command/exec/terminate":
        assert pending[0]=='standalone-test' and pending[2]==params['processId']
        send({'id':message['id'],'result':{}})
        send({'id':pending[1],'result':{'exitCode':130,'stdout':'cancelled output','stderr':''}})
        pending=None
    elif method == "turn/start":
        assert params["threadId"] == thread
        if delayed_rate_limit is not None:
            send({"id": delayed_rate_limit, "result": {"rateLimits": {"primary": {"usedPercent": 99, "windowDurationMins": 300}}}})
            delayed_rate_limit = None
        if deferred_ack is not None:
            send(deferred_ack)
            deferred_ack = None
        count += 1
        turn = f"turn-{count}"
        text = params["input"][0]["text"]
        if mode == "commands":
            assert text.split()[0].lower() not in ("/status", "/model", "/permissions", "/help", "/compact"), "client commands must not start a model turn"
            notify("thread/tokenUsage/updated", {"tokenUsage": {"total": {"totalTokens": 1234}, "last": {"totalTokens": 234}, "modelContextWindow": 128000}})
        expected = "never" if text.startswith("no-approval") or mode.startswith('memory-') or mode == "schedule" else ({"granular": {"sandbox_approval": True, "rules": False, "skill_approval": True, "request_permissions": True, "mcp_elicitations": True}} if mode == "granular" else "on-request")
        assert params["approvalPolicy"] == expected, (params["approvalPolicy"], expected)
        if mode.startswith('memory-'):
            assert params['sandboxPolicy']=={'type':'readOnly','networkAccess':False}
            assert params['outputSchema']['properties']['events']['type']=='array'
        else:
            assert "sandboxPolicy" not in params, "approval toggle must preserve sandbox permissions"
        if mode == "crash":
            sys.exit(3)
        if mode == "error" or text == "fail":
            send({"id": message["id"], "error": {"code": -32000, "message": "模拟请求失败"}})
            continue
        if text not in ("fast", "late-ack"):
            send({"id": message["id"], "result": {"turn": {"id": turn}}})
        if mode.startswith('memory-'):
            if mode=='memory-interactive':
                send({'id':'blocked','method':'item/tool/requestUserInput','params':{}})
            elif mode=='memory-malformed':
                complete('not-json')
            elif mode=='memory-fail':
                complete('not-saved','failed')
            else:
                data=json.loads(text)
                result={'events':[{'title':'温茶偏好','description':'用户说喜欢温热的茶。','category':'偏好','happenedAt':'未说明','participants':['用户',data['character']],'importance':2,'perspective':'用户陈述','evidence':data['user']}]}
                complete(json.dumps(result,ensure_ascii=False))
        elif text.startswith("visibility:"):
            from pathlib import Path
            import time
            directory = Path(text.removeprefix("visibility:"))
            for phase, delta in [("first", "首段"), ("next", "后续"), ("finish", "最终")]:
                deadline = time.monotonic() + 60
                while not (directory / phase).exists():
                    assert time.monotonic() < deadline, "visibility fixture timed out"
                    time.sleep(0.01)
                if phase == "finish":
                    complete("首段后续最终")
                else:
                    notify("item/agentMessage/delta", {"itemId": "reply-" + turn, "delta": delta})
        elif text.startswith("world-"):
            complete(json.dumps(params["input"], ensure_ascii=False))
        elif text == "resume-status":
            complete(session_method + ":" + os.getcwd())
        elif text == "execution":
            notify("item/commandExecution/outputDelta", {"itemId":"command-1","delta":"第一段\n"})
            notify("item/started", {"item":{"id":"command-1","type":"commandExecution","command":"cargo test affected","cwd":os.getcwd(),"status":"inProgress"}})
            notify("item/commandExecution/outputDelta", {"itemId":"command-1","delta":"第二段\n"})
            notify("item/completed", {"item":{"id":"command-1","type":"commandExecution","command":"cargo test affected","cwd":os.getcwd(),"status":"completed","aggregatedOutput":"第一段\n第二段\n","exitCode":0,"durationMs":25}})
            notify("turn/plan/updated", {"explanation":"验证受影响模块","plan":[{"step":"运行检查","status":"completed"}]})
            notify("item/started", {"item":{"id":"patch-1","type":"fileChange","status":"inProgress","changes":[{"path":"example.rs","kind":{"type":"update"},"diff":"-before\n+after"}]}})
            notify("item/fileChange/patchUpdated", {"itemId":"patch-1","changes":[{"path":"example.rs","kind":{"type":"update"},"diff":"-before\n+after"}]})
            notify("item/completed", {"item":{"id":"patch-1","type":"fileChange","status":"completed","changes":[{"path":"example.rs","kind":{"type":"update"},"diff":"-before\n+after"}]}})
            notify("turn/diff/updated", {"diff":"--- a/example.rs\n+++ b/example.rs\n-before\n+after"})
            complete("```rust\nfn fixed() {}\n```\n已完成受影响检查")
        elif text == "persona-status":
            complete(params.get("developerInstructions", "turn-has-no-persona") + ":" + str(count))
        elif text == "descendant":
            child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
            complete(str(child.pid))
        elif text == "approve":
            pending = "approve"
            send({"id": "approval-1", "method": "item/commandExecution/requestApproval", "params": {"threadId": thread, "turnId": turn, "reason": "运行项目检查", "command": "cargo test", "cwd": os.getcwd()}})
        elif text == "echo-secret":
            complete(os.environ.get('LEXINGTON_MODEL_API_KEY', ''))
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
    elif method == "turn/steer":
        assert params['threadId'] == thread and params['expectedTurnId'] == turn
        assert set(params) == {'threadId', 'expectedTurnId', 'input'}
        text = params['input'][0]['text']
        if text == 'reject':
            send({'id': message['id'], 'error': {'code': -32000, 'message': '补充被拒绝'}})
        elif text == 'wrong-turn':
            send({'id': message['id'], 'result': {'turnId': 'unrelated'}})
        elif text == 'disconnect':
            sys.exit(0)
        else:
            steers.append(text)
            if text == 'finish-before-ack':
                complete('完成：' + '|'.join(steers))
            send({'id': message['id'], 'result': {'turnId': turn}})
            if text == 'finish':
                complete('完成：' + '|'.join(steers))
            elif text != 'finish-before-ack':
                notify('item/agentMessage/delta', {'itemId': 'supplement-' + str(len(steers)), 'delta': '收到补充：' + text})
    elif method == "turn/interrupt":
        assert params["turnId"] == turn
        send({"id": message["id"], "result": {}})
        complete("已停止", "interrupted")
        pending = None
    elif method is None:
        if pending == "approve":
            assert message["id"] == "approval-1"
            assert message["result"]["decision"] == ("accept" if mode == "schedule" else "decline")
            complete("已拒绝命令")
        elif pending == "file-approval":
            assert message["id"] == "approval-file"
            assert message["result"]["decision"] == ("accept" if mode == "schedule" else "decline")
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
