# BPM Example Configurations

Example config files demonstrating BPM features.

## Usage

```bash
# Start a process from config
bpm start ./example/configs/01-simple.json

# List all processes
bpm list

# View logs
bpm logs simple-server

# Stop a process
bpm stop simple-server
```

## Config Files

| File | Description |
|------|-------------|
| `01-simple.json` | Minimal config - just name, script, args |
| `02-with-env.json` | Environment variables |
| `03-healthcheck-tcp.json` | TCP port health check |
| `04-healthcheck-http.json` | HTTP endpoint health check |
| `05-healthcheck-command.json` | Custom command health check |
| `06-restart-policy.json` | Restart behavior (always/on-failure/never) |
| `07-scheduled.json` | Cron-style scheduled tasks |
| `08-full-featured.json` | All options combined |
| `09-multi-app.json` | Multiple apps in one config |

## Health Check Types

### TCP
Checks if a port is open and accepting connections:
```json
"healthcheck": {
  "type": "tcp",
  "host": "127.0.0.1",
  "port": 3000,
  "interval": "30s",
  "timeout": "5s",
  "retries": 3
}
```

### HTTP
Checks if an endpoint returns 2xx/3xx status:
```json
"healthcheck": {
  "type": "http",
  "url": "http://localhost:3000/health",
  "interval": "30s",
  "timeout": "5s",
  "retries": 3
}
```

### Command
Runs a script and checks exit code (0 = healthy):
```json
"healthcheck": {
  "type": "command",
  "command": "/path/to/healthcheck.sh",
  "interval": "30s",
  "timeout": "10s",
  "retries": 3
}
```

## Restart Policies

| Policy | Behavior |
|--------|----------|
| `always` | Always restart when process exits |
| `on-failure` | Only restart on non-zero exit code |
| `never` | Never auto-restart |

## Duration Format

- Seconds: `30s`
- Minutes: `5m` or `5min`
- Hours: `1h` or `1hr`
