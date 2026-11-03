#!/usr/bin/env python3
"""Check installed entry points and start the service and Qt mixer without hardware."""

import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def wait_for_socket(process, path):
    deadline = time.monotonic() + 15
    while not path.is_socket() and process.poll() is None and time.monotonic() < deadline:
        time.sleep(0.05)
    if process.poll() is not None or not path.is_socket():
        raise RuntimeError(f"{path.name} did not become ready")


with tempfile.TemporaryDirectory(prefix="minifuse-smoke-") as directory:
    root = Path(directory)
    env = {
        **os.environ,
        "XDG_RUNTIME_DIR": str(root),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_DATA_HOME": str(root / "data"),
        "QT_QPA_PLATFORM": "offscreen",
        "QT_QUICK_BACKEND": "software",
        "QT_QUICK_CONTROLS_STYLE": "Fusion",
        "QT_QPA_PLATFORMTHEME": "",
    }
    for command in ("mf-service", "minifuse-control", "minifuse-desktop-agent"):
        subprocess.run([command, "--version"], env=env, check=True, timeout=10)
    processes = []
    try:
        service = subprocess.Popen(["mf-service"], env=env)
        processes.append(service)
        endpoint = root / "mf-service/control.sock"
        wait_for_socket(service, endpoint)
        with socket.socket(socket.AF_UNIX) as peer:
            peer.settimeout(3)
            peer.connect(str(endpoint))
            peer.sendall(b'{"api":1,"id":"smoke","method":"hello","params":{"client":"package-smoke","supported_api":[1]}}\n')
            with peer.makefile("rb") as reply:
                assert "result" in json.loads(reply.readline())
        gui = subprocess.Popen(["minifuse-control"], env=env)
        processes.append(gui)
        wait_for_socket(gui, root / "minifuse-control/activate.sock")
        subprocess.run(["minifuse-control", "--toggle"], env=env, check=True, timeout=10)
        assert gui.wait(timeout=10) == 0
        print("Installed service API and Qt mixer passed.")
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
