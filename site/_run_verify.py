import subprocess, sys
try:
    out = subprocess.run(["node", "_verify_lessons.mjs"], cwd=".", capture_output=True, timeout=60)
    sys.stdout.buffer.write(out.stdout)
    sys.stdout.buffer.write(out.stderr)
except subprocess.TimeoutExpired:
    sys.stdout.buffer.write(b"TIMEOUT\n")
