"""Exercise the compiled MCP binary over real stdio, using disposable copies only.

No GUI, remote model, or HTTP client. --photos selects at most three authorized
JPEG sources; otherwise creates small synthetic JPEGs. --models enables real
inference with operator-provided model artifacts.
"""
import argparse
import base64
import hashlib
import json
import queue
import shutil
import sqlite3
import subprocess
import threading
import time
from pathlib import Path
import psutil


class Client:
    def __init__(self, binary, data, models, root, read_only=False):
        self.stderr = (root / ("readonly-stderr.log" if read_only else "stderr.log")).open("ab")
        command = [str(binary), "--data-dir", str(data), "--model-dir", str(models), "--allow-root", str(root), "--worker-limit", "2"]
        if read_only:
            command.append("--read-only")
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
                                        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        self.messages = queue.Queue()
        self.next_id = 0
        def read():
            for line in self.process.stdout:
                try:
                    self.messages.put(json.loads(line))
                except Exception:
                    self.messages.put({"protocol_error": "stdout is not JSON-RPC"})
            self.messages.put({"eof": True})
        threading.Thread(target=read, daemon=True).start()
        self.initialization = self.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "iris-stdio-validation", "version": "1"}})
        assert self.initialization["serverInfo"]["name"] == "iris-mcp", self.initialization
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, message):
        self.process.stdin.write((json.dumps(message) + "\n").encode())
        self.process.stdin.flush()

    def request(self, method, params):
        self.next_id += 1
        request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            message = self.messages.get(timeout=max(0.1, deadline - time.monotonic()))
            assert not message.get("protocol_error") and not message.get("eof"), message
            if message.get("id") == request_id:
                assert "error" not in message, message
                return message["result"]
        raise TimeoutError(method)

    def tool(self, name, arguments=None, error=False):
        result = self.request("tools/call", {"name": name, "arguments": arguments or {}})
        assert bool(result.get("isError")) == error, (name, result)
        return result

    def value(self, name, arguments=None):
        return self.tool(name, arguments)["structuredContent"]["result"]

    def wait_job(self, project, task):
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            progress = self.value("iris_job_status", {"project_id": project})
            assert progress["id"] == task["id"], progress
            if progress["state"] not in ("running", "paused"):
                return progress
            time.sleep(0.1)
        raise TimeoutError("job did not finish")

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise AssertionError("MCP did not exit after stdin EOF")
        self.stderr.close()
        assert self.process.returncode == 0, self.process.returncode


def verify(args):
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    photos = root / "photos"
    photos.mkdir()
    if args.photos:
        sources = sorted(p for p in args.photos.resolve().iterdir() if p.suffix.lower() in (".jpg", ".jpeg"))[:3]
        assert len(sources) == 3
        for index, source in enumerate(sources):
            shutil.copy2(source, photos / f"photo-{index}.jpg")
    else:
        from PIL import Image, ImageDraw
        for index in range(3):
            image = Image.new("RGB", (320, 240), (60 + index * 30, 80, 110))
            ImageDraw.Draw(image).rectangle((60, 40, 220, 200), fill=(170, 140, 100))
            image.save(photos / f"photo-{index}.jpg")
    shutil.copy2(photos / "photo-0.jpg", photos / "duplicate.jpg")
    originals = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in photos.glob("*.jpg")}
    binary = args.binary.resolve()
    models = args.models.resolve() if args.models else root / "no-models"
    client = Client(binary, root / "data", models, root)
    checks = []
    report = {"ok": False, "fixture": str(root), "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "real_inference": bool(args.models), "server_info": client.initialization.get("serverInfo"), "checks": checks}
    try:
        tools = client.request("tools/list", {})["tools"]
        report["tools"] = len(tools)
        assert len(tools) >= 40
        checks.append("SDK handshake and tool discovery; protocol-only stdout")
        lock_check = subprocess.run([str(binary), "--data-dir", str(root / "data"), "--model-dir", str(models), "--allow-root", str(root)],
                                    input=b"", capture_output=True, timeout=15,
                                    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        assert lock_check.returncode != 0 and b"database is in use" in lock_check.stderr and not lock_check.stdout
        checks.append("concurrent process database ownership refused")
        client.tool("iris_create_project", {"request_id": "outside", "data": {"root": str(root.parent)}}, error=True)
        checks.append("outside root refused")
        create = {"request_id": "create", "data": {"root": str(photos)}}
        project = client.value("iris_create_project", create)
        assert client.value("iris_create_project", create) == project
        pid = project["id"]
        checks.append("project creation and exact write replay")
        task = client.value("iris_start_scan", {"project_id": pid, "request_id": "scan"})
        scan = client.wait_job(pid, task)
        assert scan["state"] == "completed", scan
        page = client.value("iris_list_photos", {"project_id": pid, "limit": 2})
        assert len(page) == 2, page
        photo_id = page[0]["id"]
        checks.append("asynchronous scan and bounded pagination")
        client.tool("iris_list_photos", {"project_id": pid, "limit": 101}, error=True)
        client.tool("iris_list_photos", {"project_id": pid, "unexpected": True}, error=True)
        checks.append("invalid limits and unknown tool arguments refused")
        preview = client.tool("iris_get_preview", {"photo_id": photo_id})
        image = next(c for c in preview["content"] if c["type"] == "image")
        assert image["mimeType"] == "image/jpeg" and base64.b64decode(image["data"]).startswith(b"\xff\xd8")
        checks.append("actual MCP image result")
        settings = client.value("iris_get_settings", {"project_id": pid})
        client.value("iris_set_settings", {"project_id": pid, "request_id": "settings", "data": settings})
        client.value("iris_model_status", {"project_id": pid})
        assert not [c for c in psutil.Process(client.process.pid).net_connections() if c.status == psutil.CONN_LISTEN]
        checks.append("MCP process has no listening network socket")
        if args.models:
            task = client.value("iris_start_analysis", {"project_id": pid, "request_id": "analysis"})
            analysis = client.wait_job(pid, task)
            assert analysis["state"] == "completed", analysis
            report["analysis"] = analysis
            client.value("iris_get_groups", {"project_id": pid, "limit": 2})
            checks.append("real model analysis using headless binary workers and group retrieval")
            settings["max_faces"] = 9 if settings["max_faces"] == 10 else 10
            client.value("iris_set_settings", {"project_id": pid, "request_id": "invalidate", "data": settings})
            task = client.value("iris_start_analysis", {"project_id": pid, "request_id": "cancel-analysis"})
            client.value("iris_pause_job", {"project_id": pid, "request_id": "pause"})
            client.value("iris_resume_job", {"project_id": pid, "request_id": "resume"})
            client.value("iris_cancel_job", {"project_id": pid, "request_id": "cancel"})
            cancelled = client.wait_job(pid, task)
            assert cancelled["state"] == "cancelled", cancelled
            checks.append("analysis pause/resume/cancel has terminal cancelled result")
        else:
            task = client.value("iris_start_analysis", {"project_id": pid, "request_id": "missing-models"})
            failed = client.wait_job(pid, task)
            assert failed["state"] == "failed" and failed["errors"], failed
            checks.append("missing models produce explicit task failure")
        marks = {"project_id": pid, "request_id": "marks", "data": {"photo_ids": [photo_id], "decision": "keep", "rating": 4, "color_label": "blue"}}
        result = client.value("iris_set_marks", marks)
        assert client.value("iris_set_marks", marks) == result
        assert client.value("iris_get_photo", {"photo_id": photo_id})["rating"] == 4
        client.value("iris_undo", {"project_id": pid, "request_id": "undo"})
        assert client.value("iris_get_photo", {"photo_id": photo_id})["rating"] == 0
        marks["request_id"] = "marks-2"
        client.value("iris_set_marks", marks)
        checks.append("decision/star/color atomic marks, replay and undo")
        csv = root / "decisions.csv"
        client.value("iris_export_csv", {"project_id": pid, "request_id": "csv", "data": {"destination": str(csv)}})
        assert csv.exists()
        destination = root / "selected"
        client.value("iris_export_copy", {"project_id": pid, "request_id": "copy", "data": {"destination": str(destination), "scope": "keep"}})
        assert len(list(destination.glob("*.jpg"))) == 1
        client.value("iris_export_xmp", {"project_id": pid, "request_id": "xmp", "data": {"scope": "keep"}})
        assert len(list(photos.glob("*.xmp"))) == 1
        checks.append("CSV, selected copy and XMP outputs")
        client.value("iris_set_marks", {"project_id": pid, "request_id": "reject", "data": {"photo_ids": [photo_id], "decision": "reject"}})
        plan = client.value("iris_quarantine_preview", {"project_id": pid, "request_id": "plan"})
        report["quarantine_plan"] = plan
        manifest = plan["id"]
        client.tool("iris_quarantine_commit", {"project_id": pid, "request_id": "no-confirm", "data": {"manifest_id": manifest}}, error=True)
        client.value("iris_quarantine_commit", {"project_id": pid, "request_id": "quarantine", "confirm": True, "data": {"manifest_id": manifest}})
        client.value("iris_quarantine_restore", {"project_id": pid, "request_id": "restore", "confirm": True, "data": {"manifest_id": manifest}})
        assert originals == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in photos.glob("*.jpg")}
        checks.append("manifest confirmation, quarantine/restore and unchanged source hashes")
        client.value("iris_cache_status", {"project_id": pid})
        client.value("iris_cache_migrate", {"project_id": pid, "request_id": "migrate-cache", "data": {"destination": str(root / "new-cache")}})
        migrations = client.value("iris_cache_history", {"project_id": pid})["items"]
        assert len(migrations) == 1
        client.value("iris_cache_cleanup_old", {"project_id": pid, "request_id": "cleanup-old-cache", "migration_id": migrations[0]["id"], "confirm": True})
        checks.append("cache migration and manifest-scoped old cache cleanup")
        with sqlite3.connect(root / "data/library.sqlite3") as db:
            assert db.execute("SELECT count(*) FROM decisions WHERE source='agent:mcp'").fetchone()[0] > 0
        checks.append("agent attribution in database")
        if args.models:
            settings["max_faces"] = 8
            client.value("iris_set_settings", {"project_id": pid, "request_id": "eof-settings", "data": settings})
            client.value("iris_start_analysis", {"project_id": pid, "request_id": "eof-analysis"})
            children = []
            for _ in range(100):
                children = psutil.Process(client.process.pid).children(recursive=True)
                if children:
                    break
                time.sleep(0.005)
            assert children, "no analysis worker observed before EOF test"
            assert all("iris-shell" not in p.name().lower() for p in children)
            client.close()
            _, alive = psutil.wait_procs(children, timeout=5)
            assert not alive, [(p.pid, p.name()) for p in alive]
            checks.append("stdin EOF during real analysis terminates owned workers without GUI")
        client.close()
        client = Client(binary, root / "data", models, root)
        assert client.value("iris_create_project", create) == project
        checks.append("write replay survives process restart")
        client.close()
        client = Client(binary, root / "data", models, root, read_only=True)
        readonly = client.request("tools/list", {})["tools"]
        assert readonly and all(t["annotations"]["readOnlyHint"] for t in readonly)
        client.value("iris_list_photos", {"project_id": pid})
        checks.append("read-only capability list and existing project access")
        report["ok"] = True
    finally:
        client.close()
        (root / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"ok": report["ok"], "checks": len(checks), "report": str(root / "report.json")}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/iris-mcp.exe"))
    parser.add_argument("--models", type=Path)
    parser.add_argument("--photos", type=Path)
    parser.add_argument("--output", type=Path, required=True, help="Must not already exist")
    verify(parser.parse_args())
