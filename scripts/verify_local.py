#!/usr/bin/env python3
"""Exercise the built CLI with disposable files and loopback-only update mocks."""

import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    before = hashlib.sha256(binary.read_bytes()).hexdigest()
    results = []
    with tempfile.TemporaryDirectory(prefix="nswt-cli-verification-") as directory:
        root = Path(directory)
        data = root / "isolated-data"
        env = os.environ.copy()
        env.update(NSWT_OFFLINE="1", NSWT_DATA_DIR=str(data))
        env.pop("NSWT_UPDATE_API_BASE", None)

        def run(name, arguments, code, contains, overrides=None):
            case_env = env.copy()
            if overrides:
                case_env.update(overrides)
            process = subprocess.run(
                [str(binary), *arguments], cwd=root, env=case_env,
                capture_output=True, text=True, timeout=30,
            )
            text = process.stdout + process.stderr
            (output / (name + ".log")).write_text(text, encoding="utf-8")
            passed = process.returncode == code and all(value in text for value in contains)
            results.append({"case": name, "arguments": arguments,
                            "exit_code": process.returncode, "expected_code": code,
                            "passed": passed})
            if not passed:
                raise AssertionError(f"{name}: exit {process.returncode}; {text}")

        run("help", ["--help"], 0, ["template", "--version", "--yes"])
        run("version", ["--version"], 0, ["NekoSportsWorldTool", "0.3.1"])
        run("unknown", ["--definitely-invalid"], 1, ["未知命令"])
        run("template-missing-argument", ["template"], 1, ["缺少 --file"])
        gpx = root / "路线 样本.gpx"
        gpx.write_text('<gpx><trk><trkseg><trkpt lat="1" lon="2"><ele>3</ele>'
                       '</trkpt><trkpt lat="1.01" lon="2.01"><ele>9</ele>'
                       '</trkpt></trkseg></trk></gpx>', encoding="utf-8")
        run("template-gpx-unicode-path", ["template", "--file", str(gpx)], 0,
            ["采样点：2", "3.0–9.0 m", "累计上升：6.0 m"])
        local_json = root / "sample.json"
        local_json.write_text('[{"elevation":3},{"bdA":"9"}]', encoding="utf-8")
        run("template-json", ["template", "--file", str(local_json)], 0, ["采样点：2"])
        local_json.write_text('{"bad":', encoding="utf-8")
        run("template-invalid-json", ["template", "--file", str(local_json)], 1,
            ["JSON 模板格式错误"])
        run("template-nonexistent", ["template", "--file", "missing.gpx"], 1,
            ["模板读取失败"])
        for command in ["login", "logout", "run", "ai", "ai-list", "records",
                        "records-raw", "record-info", "obs-get", "ai-records",
                        "ai-info", "semester", "cheat", "rank", "update"]:
            run("offline-" + command, [command], 1, ["离线验证模式：联网操作已禁用"])
        if data.exists():
            raise AssertionError("Local CLI/help or blocked online command created account state")

        # The only enabled HTTP transport in this suite is the explicit local Mock.
        state = {"status": 200, "body": [], "requests": []}

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                state["requests"].append(self.path)
                body = json.dumps(state["body"]).encode()
                self.send_response(state["status"])
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_arguments):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        mock_env = {"NSWT_OFFLINE": "0",
                    "NSWT_UPDATE_API_BASE": f"http://127.0.0.1:{server.server_port}"}
        try:
            run("update-empty", ["update", "--check"], 0, ["未发现更高版本"], mock_env)
            state["status"] = 404
            run("update-no-release-404", ["update", "--check"], 0, ["暂无可用发行"], mock_env)
            state["status"] = 200
            asset = "NekoSportsWorldTool-J-claude-macOS-arm64.tar.gz"
            if os.name == "nt":
                asset = "NekoSportsWorldTool-J-claude-Windows-x64.zip"
            tag = "jclaude-v99.0.0"
            repo = "1373878757-ux/NekoSportsWorldTool"
            release = {"tag_name": tag, "name": "Local verification only", "body": "",
                       "assets": [{"name": asset, "size": 10,
                                   "browser_download_url":
                                   f"https://github.com/{repo}/releases/download/{tag}/{asset}"}]}
            state["body"] = [release]
            run("update-check-compatible", ["update", "--check"], 0,
                [tag, "仅检查（--check），未下载"], mock_env)
            run("update-requires-confirmation", ["update"], 2,
                ["尚未确认", "未下载、替换或重启"], mock_env)
            release["assets"][0]["browser_download_url"] = "https://example.invalid/upstream.zip"
            run("update-rejects-other-source", ["update", "--check"], 0,
                ["未发现更高版本"], mock_env)
            state["status"] = 503
            run("update-http-error", ["update", "--check"], 1, ["检查更新失败"], mock_env)
            if any("/releases?per_page=100" not in path for path in state["requests"]):
                raise AssertionError("Unexpected download/replacement request during checks")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        run("update-rejects-public-mock-base", ["update", "--check"], 1,
            ["仅允许带端口的本机 HTTP Mock 地址"],
            {"NSWT_OFFLINE": "0", "NSWT_UPDATE_API_BASE": "https://example.invalid"})
        if data.exists():
            raise AssertionError("Update checks wrote account state")
    after = hashlib.sha256(binary.read_bytes()).hexdigest()
    if before != after:
        raise AssertionError("CLI checks replaced the executable")
    report = {"binary": str(binary), "binary_sha256": after,
              "private_data_created": False, "update_download_or_replacement": False,
              "passed": len(results), "cases": results}
    (output / "cli-smoke.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                           encoding="utf-8")
    print(f"{len(results)} CLI/offline/loopback cases passed; executable unchanged")


if __name__ == "__main__":
    main()
