"""Shared local daemon harness and sampled process-tree memory evidence."""
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time
import urllib.request
import psutil


class Daemon:
    def __init__(self, executable, models, directory, workers=2, env=None):
        self.directory = Path(directory).resolve()
        self.directory.mkdir(parents=True, exist_ok=True)
        self.log = (self.directory / "daemon.log").open("w", encoding="utf-8")
        self.process = subprocess.Popen([str(Path(executable).resolve()), "--data-dir", str(self.directory / "data"),
            "--model-dir", str(Path(models).resolve()), "--worker-limit", str(workers)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True, env={**os.environ, **(env or {})})
        self.stop = threading.Event()
        self.memory_samples = []
        self.memory = {"interval_ms": 100, "peak_tree_rss_bytes": 0, "peak_tree_private_bytes": 0, "peak_processes": 0, "samples": 0, "missed_process_samples": 0,
            "scope": "daemon and recursive child processes; sampled sum, shared pages may be counted repeatedly; not GPU VRAM"}
        self.monitor = threading.Thread(target=self._monitor, daemon=True)
        self.monitor.start()
        lines = queue.Queue()
        threading.Thread(target=lambda: lines.put(self.process.stdout.readline()), daemon=True).start()
        try:
            self.handshake = json.loads(lines.get(timeout=30))
        except Exception:
            self.close()
            raise

    def _monitor(self):
        parent = psutil.Process(self.process.pid)
        while not self.stop.wait(.1):
            rss = private = 0
            try:
                processes = [parent] + parent.children(recursive=True)
                for process in processes:
                    try:
                        info = process.memory_info()
                        rss += info.rss
                        private += info.vms if os.name == "nt" else getattr(info, "private", 0)
                    except (psutil.NoSuchProcess, psutil.AccessDenied):
                        self.memory["missed_process_samples"] += 1
                self.memory["samples"] += 1
                self.memory["peak_tree_rss_bytes"] = max(rss, self.memory["peak_tree_rss_bytes"])
                self.memory["peak_tree_private_bytes"] = max(private, self.memory["peak_tree_private_bytes"])
                self.memory["peak_processes"] = max(len(processes), self.memory["peak_processes"])
                self.memory_samples.append({"seconds": time.monotonic(), "rss_bytes": rss,
                    "private_bytes": private, "processes": len(processes)})
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                return

    def request(self, method, path, body=None, raw=False):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(self.handshake["base_url"] + "/api/v1" + path, data=data, method=method,
            headers={"Authorization": "Bearer " + self.handshake["token"], "Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=180) as response:
            payload = response.read()
            return (response.headers.get_content_type(), payload) if raw else json.loads(payload)

    def job(self, project, kind, expected="completed", timeout=1800):
        started = time.monotonic()
        self.request("POST", f"/projects/{project}/{kind}")
        while time.monotonic() - started < timeout:
            progress = self.request("GET", f"/projects/{project}/progress")
            if progress["state"] in {"completed", "failed", "cancelled"}:
                result = {**progress, "seconds": time.monotonic() - started}
                if expected is not None:
                    assert progress["state"] == expected, result
                return result
            time.sleep(.2)
        raise TimeoutError(f"{kind} exceeded {timeout}s")

    def close(self):
        if self.process.poll() is None:
            try:
                self.process.stdin.write("shutdown\n")
                self.process.stdin.flush()
            except BrokenPipeError:
                pass
            self.process.stdin.close()
            try:
                self.process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                children = psutil.Process(self.process.pid).children(recursive=True)
                for child in children:
                    try: child.kill()
                    except psutil.NoSuchProcess: pass
                self.process.kill()
                self.process.wait(timeout=10)
        self.stop.set()
        self.monitor.join(timeout=2)
        self.process.stdout.close()
        self.log.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
