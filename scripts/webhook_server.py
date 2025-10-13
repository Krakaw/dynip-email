#!/usr/bin/env python3
"""
Simple webhook test server for debugging webhook failures.
Run this script and configure your webhook to point to http://localhost:3009
"""

import json
import http.server
import socketserver
from datetime import datetime

class WebhookHandler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        # Read the request body
        content_length = int(self.headers.get('Content-Length', 0))
        post_data = self.rfile.read(content_length)
        
        try:
            # Parse JSON payload
            payload = json.loads(post_data.decode('utf-8'))
            
            print(f"\n{'='*60}")
            print(f"📨 Webhook received at {datetime.now().isoformat()}")
            print(f"🌐 Path: {self.path}")
            print(f"📋 Headers: {dict(self.headers)}")
            print(f"📦 Payload:")
            print(json.dumps(payload, indent=2))
            print(f"{'='*60}\n")
            
            # Send success response
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            
            response = {
                "status": "success",
                "message": "Webhook received successfully",
                "timestamp": datetime.now().isoformat()
            }
            
            self.wfile.write(json.dumps(response).encode('utf-8'))
            
        except json.JSONDecodeError as e:
            print(f"❌ Failed to parse JSON: {e}")
            self.send_response(400)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            
            error_response = {
                "status": "error",
                "message": f"Invalid JSON: {e}",
                "timestamp": datetime.now().isoformat()
            }
            
            self.wfile.write(json.dumps(error_response).encode('utf-8'))
            
        except Exception as e:
            print(f"❌ Unexpected error: {e}")
            self.send_response(500)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            
            error_response = {
                "status": "error",
                "message": f"Server error: {e}",
                "timestamp": datetime.now().isoformat()
            }
            
            self.wfile.write(json.dumps(error_response).encode('utf-8'))

    def log_message(self, format, *args):
        # Suppress default logging
        pass

if __name__ == "__main__":
    PORT = 3009
    
    print(f"🚀 Starting webhook test server on port {PORT}")
    print(f"📡 Configure your webhook to: http://localhost:{PORT}")
    print(f"🛑 Press Ctrl+C to stop")
    print(f"{'='*60}")
    
    with socketserver.TCPServer(("", PORT), WebhookHandler) as httpd:
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print(f"\n🛑 Server stopped")
