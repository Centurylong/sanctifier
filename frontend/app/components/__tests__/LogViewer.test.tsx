/**
 * @jest-environment jsdom
 */

import { render, screen, waitFor } from '@testing-library/react';
import { LogViewer } from '../LogViewer';

// Mock WebSocket
class MockWebSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(public url: string) {
    setTimeout(() => {
      this.onopen?.();
    }, 0);
  }

  send(data: string) {
    // Mock send
  }

  close() {
    setTimeout(() => {
      this.onclose?.();
    }, 0);
  }
}

global.WebSocket = MockWebSocket as any;

describe('LogViewer', () => {
  it('renders connection status', async () => {
    render(<LogViewer wsUrl="ws://localhost:9001" />);
    
    await waitFor(() => {
      expect(screen.getByText('Connected')).toBeInTheDocument();
    });
  });

  it('displays log messages', async () => {
    const { container } = render(<LogViewer wsUrl="ws://localhost:9001" />);
    
    await waitFor(() => {
      expect(screen.getByText('Connected')).toBeInTheDocument();
    });

    // Simulate receiving a log message
    const ws = (global.WebSocket as any).mock?.instances?.[0];
    if (ws?.onmessage) {
      ws.onmessage({
        data: JSON.stringify({
          type: 'info',
          message: 'Test message',
          timestamp: Date.now()
        })
      });
    }

    await waitFor(() => {
      expect(screen.getByText('Test message')).toBeInTheDocument();
    });
  });

  it('handles clear button', async () => {
    render(<LogViewer wsUrl="ws://localhost:9001" />);
    
    await waitFor(() => {
      expect(screen.getByText('Connected')).toBeInTheDocument();
    });

    const clearButton = screen.getByText('Clear');
    expect(clearButton).toBeInTheDocument();
  });
});
