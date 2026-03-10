# Real-time Analysis Log Streamer

The Real-time Analysis Log Streamer provides live visibility into Sanctifier's contract analysis process through WebSocket-based streaming.

## Overview

This feature allows developers to:
- Watch analysis progress in real-time
- See which files are being analyzed
- Monitor for issues as they're discovered
- View the final analysis report immediately upon completion

## Architecture

### Backend (CLI)

The streaming functionality is implemented in the Sanctifier CLI with the following components:

1. **WebSocket Server** (`tooling/sanctifier-cli/src/ws_server.rs`)
   - Handles WebSocket connections on a configurable port (default: 9001)
   - Broadcasts log events to all connected clients
   - Supports multiple simultaneous connections

2. **Stream Command** (`tooling/sanctifier-cli/src/commands/stream.rs`)
   - Performs contract analysis with progress logging
   - Emits structured log events at each analysis stage
   - Sends final JSON report when complete

3. **Log Events**
   - `info`: General information messages
   - `progress`: Progress updates with current/total counts
   - `warning`: Non-critical issues
   - `error`: Critical errors
   - `complete`: Analysis completion
   - `file_analysis`: Per-file analysis status

### Frontend (Next.js)

The frontend provides a real-time log viewer with:

1. **LogViewer Component** (`frontend/app/components/LogViewer.tsx`)
   - Connects to WebSocket server
   - Displays logs with color-coded severity
   - Auto-scrolls to latest messages
   - Parses final report for visualization

2. **Stream Page** (`frontend/app/stream/page.tsx`)
   - Configuration UI for WebSocket connection
   - Live log display
   - Real-time findings visualization
   - Summary charts

## Usage

### Starting the Analysis Server

Run the Sanctifier CLI with the `stream` command:

```bash
# Analyze current directory
sanctifier stream

# Analyze specific path
sanctifier stream /path/to/contract

# Use custom port
sanctifier stream --port 9002

# Set custom ledger limit
sanctifier stream --limit 128000
```

### Connecting from Frontend

1. Start the Next.js development server:
   ```bash
   cd frontend
   npm run dev
   ```

2. Navigate to `http://localhost:3000/stream`

3. Configure the WebSocket URL (default: `ws://localhost:9001`)

4. Click "Start Streaming"

5. In another terminal, run the analysis:
   ```bash
   sanctifier stream /path/to/contract
   ```

### Programmatic Usage

You can also connect to the WebSocket server from any client:

```javascript
const ws = new WebSocket('ws://localhost:9001');

ws.onmessage = (event) => {
  const logEntry = JSON.parse(event.data);
  console.log(logEntry);
};
```

## Log Event Schema

All log events follow this TypeScript interface:

```typescript
interface LogEntry {
  type: "info" | "progress" | "warning" | "error" | "complete" | "file_analysis";
  message?: string;
  current?: number;  // For progress events
  total?: number;    // For progress events
  file?: string;     // For file_analysis events
  status?: string;   // For file_analysis events
  timestamp: number; // Unix timestamp in milliseconds
}
```

### Example Events

**Info Event:**
```json
{
  "type": "info",
  "message": "Starting Sanctifier analysis...",
  "timestamp": 1678901234567
}
```

**Progress Event:**
```json
{
  "type": "progress",
  "message": "Analyzing src/lib.rs",
  "current": 3,
  "total": 10,
  "timestamp": 1678901234567
}
```

**File Analysis Event:**
```json
{
  "type": "file_analysis",
  "file": "src/contract.rs",
  "status": "analyzing",
  "timestamp": 1678901234567
}
```

**Complete Event:**
```json
{
  "type": "complete",
  "message": "Analysis complete",
  "timestamp": 1678901234567
}
```

## Configuration

### CLI Options

- `--port, -p`: WebSocket server port (default: 9001)
- `--limit, -l`: Ledger entry size limit in bytes (default: 64000)

### Frontend Configuration

The WebSocket URL can be configured in the UI. For production deployments, you may want to:

1. Use environment variables:
   ```javascript
   const wsUrl = process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:9001';
   ```

2. Support secure WebSocket (wss://) for HTTPS deployments

## Testing

Run the test suite:

```bash
# Backend tests
cd tooling/sanctifier-cli
cargo test stream_tests

# Frontend tests
cd frontend
npm test
```

## Troubleshooting

### Connection Refused

If you see "WebSocket connection error":
1. Ensure the CLI server is running: `sanctifier stream`
2. Check the port is not in use: `netstat -an | grep 9001`
3. Verify firewall settings allow the connection

### No Logs Appearing

If connected but no logs appear:
1. Ensure you're analyzing a valid Soroban project
2. Check the CLI terminal for error messages
3. Verify the path contains `.rs` files

### Performance Issues

For large projects:
1. The WebSocket buffer size is set to 100 events
2. Consider increasing it in `ws_server.rs` if needed
3. Use the regular `analyze` command for very large codebases

## Future Enhancements

Potential improvements for future versions:

- [ ] Authentication/authorization for WebSocket connections
- [ ] Multiple analysis sessions with session IDs
- [ ] Pause/resume analysis capability
- [ ] Export logs to file
- [ ] Real-time metrics dashboard
- [ ] Integration with CI/CD pipelines
- [ ] Slack/Discord notifications

## Security Considerations

- The WebSocket server binds to `127.0.0.1` by default (localhost only)
- For remote access, use a reverse proxy with authentication
- Consider rate limiting for production deployments
- Validate all incoming WebSocket messages
- Use WSS (WebSocket Secure) in production environments
